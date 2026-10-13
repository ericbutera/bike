//! Test-only adapter to the pinned SDK's native workflow runner.
//! Activities and children receive fixture results; no worker or server starts.
use bike_core::strava::events::StravaEvent;
use serde::{de::DeserializeOwned, Serialize};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, task::Waker};
use temporalio_common::{
    data_converters::DataConverter,
    protos::{
        coresdk::{
            activity_result, child_workflow,
            workflow_activation::{
                self, workflow_activation_job::Variant, WorkflowActivation, WorkflowActivationJob,
            },
            workflow_commands::{workflow_command, WorkflowCommand},
        },
        temporal::api::{
            common::v1::Payload,
            failure::v1::{failure::FailureInfo, ApplicationFailureInfo, Failure},
        },
    },
};
use temporalio_workflow::{
    __private::sdk::{
        GuestWorkflowInstance, MainRoutineCompletion, RoutineCompletion,
        TerminalOutcome as WorkflowCompletion, WorkflowHost, WorkflowImplementation, WorkflowInit,
        WorkflowInstance, MAIN_ROUTINE_ID,
    },
    common::WorkflowDefinition,
    BaseWorkflowContext,
};

#[derive(Default)]
struct Commands(RefCell<VecDeque<WorkflowCommand>>);
impl WorkflowHost for Commands {
    fn set_current_details(&self, _: String) {}
    fn push_command(&self, command: WorkflowCommand) {
        self.0.borrow_mut().push_back(command);
    }
}

pub struct WorkflowMock {
    instance: Box<dyn WorkflowInstance>,
    commands: Rc<Commands>,
    completion: Option<WorkflowCompletion>,
    replaying: bool,
}
impl WorkflowMock {
    pub fn new<W>(input: <W::Run as WorkflowDefinition>::Input) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        Self::with_replay::<W>(input, false)
    }

    pub fn with_replay<W>(input: <W::Run as WorkflowDefinition>::Input, replaying: bool) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        let commands = Rc::new(Commands::default());
        let context = BaseWorkflowContext::from_raw(
            WorkflowInit {
                namespace: "bike".into(),
                task_queue: "unit-test".into(),
                run_id: "00000000-0000-4000-8000-000000000099".into(),
                initialize_workflow: workflow_activation::InitializeWorkflow {
                    workflow_id: "unit-workflow".into(),
                    workflow_type: W::name().into(),
                    ..Default::default()
                },
            },
            DataConverter::default(),
            commands.clone(),
            None,
            vec![],
        );
        let instance =
            GuestWorkflowInstance::<W>::new_with_workflow(W::default(), context, Some(input));
        let mut mock = Self {
            instance: Box::new(instance),
            commands,
            completion: None,
            replaying,
        };
        mock.set_replaying(replaying);
        mock.poll();
        mock
    }

    pub fn set_replaying(&mut self, replaying: bool) {
        self.replaying = replaying;
        self.instance
            .activate(
                WorkflowActivation {
                    is_replaying: replaying,
                    ..Default::default()
                },
                Waker::noop(),
            )
            .unwrap();
    }

    pub fn activity(&mut self, name: &str, input: &impl Serialize, output: &impl Serialize) {
        let (seq, arguments) = self.next_activity(name);
        assert_input(&arguments, input);
        self.resolve_activity(
            seq,
            activity_result::activity_resolution::Status::Completed(activity_result::Success {
                result: Some(payload(output)),
            }),
        );
    }

    pub fn fail_activity(&mut self, name: &str, message: &str) {
        let (seq, _) = self.next_activity(name);
        self.resolve_activity(
            seq,
            activity_result::activity_resolution::Status::Failed(activity_result::Failure {
                failure: Some(failure(message)),
                ..Default::default()
            }),
        );
    }

    fn next_activity(&mut self, name: &str) -> (u32, Vec<Payload>) {
        let command = self.pop_command();
        let summary = command
            .user_metadata
            .as_ref()
            .unwrap()
            .summary
            .as_ref()
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<String>(&summary.data).unwrap(),
            name.strip_prefix("strava_")
                .unwrap_or(name)
                .replace('_', " ")
        );
        let workflow_command::Variant::ScheduleActivity(command) = command.variant.unwrap() else {
            panic!("Expected activity {name}")
        };
        assert_eq!(command.activity_type, name);
        assert_eq!(command.retry_policy.unwrap().maximum_attempts, 12);
        (command.seq, command.arguments)
    }

    fn resolve_activity(&mut self, seq: u32, status: activity_result::activity_resolution::Status) {
        self.activate(Variant::ResolveActivity(
            workflow_activation::ResolveActivity {
                seq,
                result: Some(activity_result::ActivityResolution {
                    status: Some(status),
                }),
                ..Default::default()
            },
        ));
    }

    pub fn start_child(&mut self, name: &str, input: &StravaEvent) -> u32 {
        let workflow_command::Variant::StartChildWorkflowExecution(command) = self.next_command()
        else {
            panic!("Expected child {name}")
        };
        assert_eq!(command.workflow_type, name);
        assert_input(&command.input, input);
        let attributes = command.search_attributes.as_ref().unwrap();
        for (key, expected) in [
            ("BikeCustomerId", input.bike_customer_id.to_string()),
            ("BikeUserId", input.bike_user_id.to_string()),
            ("StravaAthleteId", input.strava_athlete_id.to_string()),
            ("StravaEventId", input.strava_event_id.to_string()),
            ("CorrelationId", input.correlation_id.to_string()),
        ] {
            assert_eq!(
                serde_json::from_slice::<String>(&attributes.indexed_fields[key].data).unwrap(),
                expected
            );
        }
        if let Some(id) = input.strava_activity_id {
            assert_eq!(
                serde_json::from_slice::<String>(
                    &attributes.indexed_fields["StravaActivityId"].data
                )
                .unwrap(),
                id.to_string()
            );
        }
        let seq = command.seq;
        self.activate(Variant::ResolveChildWorkflowExecutionStart(
            workflow_activation::ResolveChildWorkflowExecutionStart {
                seq,
                status: Some(
                    workflow_activation::resolve_child_workflow_execution_start::Status::Succeeded(
                        workflow_activation::ResolveChildWorkflowExecutionStartSuccess {
                            run_id: format!("child-{seq}"),
                        },
                    ),
                ),
            },
        ));
        seq
    }

    pub fn complete_child(&mut self, seq: u32, output: &impl Serialize) {
        self.resolve_child(
            seq,
            child_workflow::child_workflow_result::Status::Completed(child_workflow::Success {
                result: Some(payload(output)),
            }),
        );
    }
    pub fn fail_child(&mut self, seq: u32) {
        self.resolve_child(
            seq,
            child_workflow::child_workflow_result::Status::Failed(child_workflow::Failure {
                failure: Some(failure("Fixture child failed")),
            }),
        );
    }
    fn resolve_child(&mut self, seq: u32, status: child_workflow::child_workflow_result::Status) {
        self.activate(Variant::ResolveChildWorkflowExecution(
            workflow_activation::ResolveChildWorkflowExecution {
                seq,
                result: Some(child_workflow::ChildWorkflowResult {
                    status: Some(status),
                }),
            },
        ));
    }

    pub fn cancel(&mut self) {
        self.activate(Variant::CancelWorkflow(
            workflow_activation::CancelWorkflow {
                reason: "Fixture cancellation".into(),
            },
        ));
    }
    pub fn cancel_activity(&mut self, name: &str) {
        let (seq, _) = self.next_activity(name);
        self.cancel();
        assert!(matches!(
            self.next_command(),
            workflow_command::Variant::RequestCancelActivity(_)
        ));
        let failure = Failure {
            failure_info: Some(FailureInfo::CanceledFailureInfo(Default::default())),
            ..Default::default()
        };
        self.resolve_activity(
            seq,
            activity_result::activity_resolution::Status::Cancelled(
                activity_result::Cancellation {
                    failure: Some(failure),
                },
            ),
        );
    }

    pub fn result<T: DeserializeOwned>(&self) -> T {
        self.assert_no_commands();
        let Some(WorkflowCompletion::Completed(result)) = &self.completion else {
            panic!("Expected success, got {:?}", self.completion)
        };
        serde_json::from_slice(if result.data.is_empty() {
            b"null"
        } else {
            &result.data
        })
        .unwrap()
    }
    pub fn assert_failed(&self) {
        self.assert_no_commands();
        assert!(matches!(
            self.completion,
            Some(WorkflowCompletion::Failed(_))
        ));
    }
    pub fn assert_cancelled(&self) {
        self.assert_no_commands();
        assert!(matches!(
            self.completion,
            Some(WorkflowCompletion::Cancelled(_))
        ));
    }
    pub fn continued_input<T: DeserializeOwned>(&self) -> T {
        self.assert_no_commands();
        let Some(WorkflowCompletion::ContinueAsNew(command)) = &self.completion else {
            panic!("Expected continue-as-new")
        };
        serde_json::from_slice(&command.arguments[0].data).unwrap()
    }
    pub fn assert_no_commands(&self) {
        assert!(
            self.commands.0.borrow().is_empty(),
            "Unexpected activity or child"
        );
    }

    fn next_command(&mut self) -> workflow_command::Variant {
        self.pop_command().variant.unwrap()
    }
    fn pop_command(&mut self) -> WorkflowCommand {
        self.commands
            .0
            .borrow_mut()
            .pop_front()
            .expect("Expected a mocked dependency call")
    }
    fn activate(&mut self, variant: Variant) {
        self.instance
            .activate(
                WorkflowActivation {
                    is_replaying: self.replaying,
                    jobs: vec![WorkflowActivationJob {
                        variant: Some(variant),
                    }],
                    ..Default::default()
                },
                Waker::noop(),
            )
            .unwrap();
        self.poll();
    }
    fn poll(&mut self) {
        if let Some(RoutineCompletion::Main(MainRoutineCompletion::Terminal(completion))) = self
            .instance
            .poll_routine(MAIN_ROUTINE_ID, Waker::noop())
            .unwrap()
            .completion
        {
            self.completion = Some(*completion);
        }
    }
}

fn payload(value: &impl Serialize) -> Payload {
    Payload {
        metadata: [("encoding".into(), b"json/plain".to_vec())].into(),
        data: serde_json::to_vec(value).unwrap(),
        ..Default::default()
    }
}
fn assert_input(actual: &[Payload], expected: &impl Serialize) {
    assert_eq!(actual.len(), 1);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&actual[0].data).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
fn failure(message: &str) -> Failure {
    Failure {
        message: message.into(),
        failure_info: Some(FailureInfo::ApplicationFailureInfo(
            ApplicationFailureInfo {
                non_retryable: true,
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}
