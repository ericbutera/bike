//! Test-only adapter to the pinned SDK's native workflow runner.
//! Activities and children receive fixture results; no worker or server starts.
use serde::{de::DeserializeOwned, Serialize};
use std::{cell::RefCell, collections::VecDeque, marker::PhantomData, rc::Rc, task::Waker};
use temporalio_common::{
    data_converters::DataConverter,
    protos::{
        coresdk::{
            activity_result, child_workflow,
            workflow_activation::{self, workflow_activation_job::Variant, WorkflowActivation, WorkflowActivationJob},
            workflow_commands::{workflow_command, StartChildWorkflowExecution, WorkflowCommand},
        },
        temporal::api::{
            common::v1::Payload,
            failure::v1::{failure::FailureInfo, ApplicationFailureInfo, Failure},
        },
    },
};
use temporalio_workflow::{
    __private::sdk::{
        GuestWorkflowInstance, MainRoutineCompletion, RoutineCompletion, TerminalOutcome as WorkflowCompletion,
        WorkflowHost, WorkflowImplementation, WorkflowInit, WorkflowInstance, MAIN_ROUTINE_ID,
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

pub trait WorkflowAssertions {
    fn activity(_name: &str, _command: &WorkflowCommand) {}
    fn child(_input: &serde_json::Value, _command: &StartChildWorkflowExecution) {}
}

pub struct NoAssertions;
impl WorkflowAssertions for NoAssertions {}

pub struct WorkflowMock<A: WorkflowAssertions = NoAssertions> {
    instance: Box<dyn WorkflowInstance>,
    commands: Rc<Commands>,
    completion: Option<WorkflowCompletion>,
    replaying: bool,
    activations: Vec<WorkflowActivation>,
    issued: Vec<WorkflowCommand>,
    continue_as_new_suggested: bool,
    assertions: PhantomData<A>,
    initial_time: std::time::SystemTime,
    current_time: std::time::SystemTime,
}
impl<A: WorkflowAssertions> WorkflowMock<A> {
    pub fn new<W>(input: <W::Run as WorkflowDefinition>::Input) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        Self::with_replay::<W>(input, false)
    }

    /// A start and cancellation delivered together before a worker executes code.
    pub fn cancelled_at_start<W>(input: <W::Run as WorkflowDefinition>::Input) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        Self::with_patches::<W>(
            input,
            false,
            vec![workflow_activation::WorkflowActivationJob {
                variant: Some(Variant::CancelWorkflow(workflow_activation::CancelWorkflow {
                    reason: "Canceled before processing".into(),
                })),
            }],
            std::time::UNIX_EPOCH,
        )
    }

    pub fn with_replay<W>(input: <W::Run as WorkflowDefinition>::Input, replaying: bool) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        Self::with_patches::<W>(input, replaying, vec![], std::time::UNIX_EPOCH)
    }

    pub fn at_time<W>(input: <W::Run as WorkflowDefinition>::Input, at: std::time::SystemTime) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        Self::with_patches::<W>(input, false, vec![], at)
    }

    fn with_patches<W>(
        input: <W::Run as WorkflowDefinition>::Input,
        replaying: bool,
        patches: Vec<WorkflowActivationJob>,
        initial_time: std::time::SystemTime,
    ) -> Self
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
        let instance = GuestWorkflowInstance::<W>::new_with_workflow(W::default(), context, Some(input));
        let mut mock = Self {
            instance: Box::new(instance),
            commands,
            completion: None,
            replaying,
            activations: vec![],
            issued: vec![],
            continue_as_new_suggested: false,
            assertions: PhantomData,
            initial_time,
            current_time: initial_time,
        };
        mock.set_replaying(replaying);
        mock.instance
            .activate(
                WorkflowActivation {
                    timestamp: Some(initial_time.into()),
                    is_replaying: replaying,
                    jobs: patches,
                    ..Default::default()
                },
                Waker::noop(),
            )
            .unwrap();
        mock.poll();
        mock
    }

    pub fn set_replaying(&mut self, replaying: bool) {
        self.replaying = replaying;
        self.instance
            .activate(
                WorkflowActivation {
                    timestamp: Some(self.current_time.into()),
                    is_replaying: replaying,
                    continue_as_new_suggested: self.continue_as_new_suggested,
                    ..Default::default()
                },
                Waker::noop(),
            )
            .unwrap();
    }

    /// Reconstruct an evicted worker's state from recorded completions.
    /// Historical commands must match exactly; dependencies are never called.
    pub fn restart<W>(self, input: <W::Run as WorkflowDefinition>::Input) -> Self
    where
        W: WorkflowImplementation + Default,
        <W::Run as WorkflowDefinition>::Input: Send,
    {
        assert!(self.completion.is_none(), "Only restart an unfinished workflow");
        let patches = self
            .issued
            .iter()
            .filter_map(|command| match &command.variant {
                Some(workflow_command::Variant::SetPatchMarker(marker)) => Some(WorkflowActivationJob {
                    variant: Some(Variant::NotifyHasPatch(workflow_activation::NotifyHasPatch {
                        patch_id: marker.patch_id.clone(),
                    })),
                }),
                _ => None,
            })
            .collect();
        let mut restarted = Self::with_patches::<W>(input, true, patches, self.initial_time);
        for mut activation in self.activations {
            activation.is_replaying = true;
            restarted.apply_activation(activation);
        }
        for expected in self.issued {
            assert_eq!(restarted.pop_command(), expected, "Replay changed a command");
        }
        assert_eq!(
            *restarted.commands.0.borrow(),
            *self.commands.0.borrow(),
            "Replay changed pending work"
        );
        restarted.set_replaying(false);
        restarted
    }

    pub fn assert_failure_message(&self, expected: &str) {
        self.assert_failed();
        let Some(WorkflowCompletion::Failed(failure)) = &self.completion else {
            unreachable!()
        };
        assert!(failure.message.contains(expected), "{}", failure.message);
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
        A::activity(name, &command);
        let workflow_command::Variant::ScheduleActivity(command) = command.variant.unwrap() else {
            panic!("Expected activity {name}")
        };
        assert_eq!(command.activity_type, name);
        (command.seq, command.arguments)
    }

    fn resolve_activity(&mut self, seq: u32, status: activity_result::activity_resolution::Status) {
        self.activate(Variant::ResolveActivity(workflow_activation::ResolveActivity {
            seq,
            result: Some(activity_result::ActivityResolution { status: Some(status) }),
            ..Default::default()
        }));
    }

    fn child_command(&mut self, name: &str, input: &impl Serialize) -> u32 {
        let workflow_command::Variant::StartChildWorkflowExecution(command) = self.next_command() else {
            panic!("Expected child {name}")
        };
        assert_eq!(command.workflow_type, name);
        assert_input(&command.input, input);
        A::child(&serde_json::to_value(input).unwrap(), &command);
        command.seq
    }

    pub fn start_child(&mut self, name: &str, input: &impl Serialize) -> u32 {
        let seq = self.child_command(name, input);
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

    pub fn fail_child_start(&mut self, name: &str, input: &impl Serialize) {
        let seq = self.child_command(name, input);
        self.activate(Variant::ResolveChildWorkflowExecutionStart(
            workflow_activation::ResolveChildWorkflowExecutionStart {
                seq,
                status: Some(
                    workflow_activation::resolve_child_workflow_execution_start::Status::Failed(
                        workflow_activation::ResolveChildWorkflowExecutionStartFailure {
                            cause: child_workflow::StartChildWorkflowExecutionFailedCause::WorkflowAlreadyExists as i32,
                            ..Default::default()
                        },
                    ),
                ),
            },
        ));
    }

    pub fn suggest_continue_as_new(&mut self) {
        self.apply_activation(WorkflowActivation {
            is_replaying: self.replaying,
            continue_as_new_suggested: true,
            ..Default::default()
        });
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
    pub fn cancel_child(&mut self, seq: u32) {
        self.cancel();
        assert!(matches!(
            self.next_command(),
            workflow_command::Variant::CancelChildWorkflowExecution(_)
        ));
        let failure = Failure {
            failure_info: Some(FailureInfo::ChildWorkflowExecutionFailureInfo(Default::default())),
            cause: Some(Box::new(Failure {
                failure_info: Some(FailureInfo::CanceledFailureInfo(Default::default())),
                ..Default::default()
            })),
            ..Default::default()
        };
        self.resolve_child(
            seq,
            child_workflow::child_workflow_result::Status::Cancelled(child_workflow::Cancellation {
                failure: Some(failure),
            }),
        );
    }
    fn resolve_child(&mut self, seq: u32, status: child_workflow::child_workflow_result::Status) {
        self.activate(Variant::ResolveChildWorkflowExecution(
            workflow_activation::ResolveChildWorkflowExecution {
                seq,
                result: Some(child_workflow::ChildWorkflowResult { status: Some(status) }),
            },
        ));
    }

    pub fn cancel(&mut self) {
        self.activate(Variant::CancelWorkflow(workflow_activation::CancelWorkflow {
            reason: "Fixture cancellation".into(),
        }));
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
            activity_result::activity_resolution::Status::Cancelled(activity_result::Cancellation {
                failure: Some(failure),
            }),
        );
    }

    pub fn result<T: DeserializeOwned>(&self) -> T {
        self.assert_no_commands();
        let Some(WorkflowCompletion::Completed(result)) = &self.completion else {
            panic!("Expected success, got {:?}", self.completion)
        };
        serde_json::from_slice(if result.data.is_empty() { b"null" } else { &result.data }).unwrap()
    }
    pub fn assert_failed(&self) {
        self.assert_no_commands();
        assert!(matches!(self.completion, Some(WorkflowCompletion::Failed(_))));
    }
    pub fn assert_cancelled(&self) {
        self.assert_no_commands();
        assert!(matches!(self.completion, Some(WorkflowCompletion::Cancelled(_))));
    }
    pub fn continued_input<T: DeserializeOwned>(&self) -> T {
        self.assert_no_commands();
        let Some(WorkflowCompletion::ContinueAsNew(command)) = &self.completion else {
            panic!("Expected continue-as-new")
        };
        serde_json::from_slice(&command.arguments[0].data).unwrap()
    }
    pub fn assert_no_commands(&self) {
        assert!(self.commands.0.borrow().is_empty(), "Unexpected activity or child");
    }

    pub fn next_command(&mut self) -> workflow_command::Variant {
        self.pop_command().variant.unwrap()
    }
    pub fn pop_command(&mut self) -> WorkflowCommand {
        let command = self
            .commands
            .0
            .borrow_mut()
            .pop_front()
            .expect("Expected a mocked dependency call");
        self.issued.push(command.clone());
        command
    }
    pub fn activate(&mut self, variant: Variant) {
        self.activate_at(variant, self.current_time);
    }

    pub fn activate_at(&mut self, variant: Variant, at: std::time::SystemTime) {
        self.apply_activation(WorkflowActivation {
            timestamp: Some(at.into()),
            is_replaying: self.replaying,
            continue_as_new_suggested: self.continue_as_new_suggested,
            jobs: vec![WorkflowActivationJob { variant: Some(variant) }],
            ..Default::default()
        });
    }
    pub fn apply_activation(&mut self, mut activation: WorkflowActivation) {
        if let Some(timestamp) = &activation.timestamp {
            self.current_time = (*timestamp).try_into().unwrap();
        } else {
            activation.timestamp = Some(self.current_time.into());
        }
        self.continue_as_new_suggested = activation.continue_as_new_suggested;
        self.activations.push(activation.clone());
        self.instance.activate(activation, Waker::noop()).unwrap();
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
        while self
            .commands
            .0
            .borrow()
            .front()
            .is_some_and(|command| matches!(command.variant, Some(workflow_command::Variant::SetPatchMarker(_))))
        {
            self.pop_command();
        }
    }
}

pub fn payload(value: &impl Serialize) -> Payload {
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
        failure_info: Some(FailureInfo::ApplicationFailureInfo(ApplicationFailureInfo {
            non_retryable: true,
            ..Default::default()
        })),
        ..Default::default()
    }
}
