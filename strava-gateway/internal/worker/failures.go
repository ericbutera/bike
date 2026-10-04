package worker

import (
	"encoding/json"
	"fmt"

	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
)

// Invalid provider bodies cannot be interpreted by this version of the app.
// Keep the exact response in protected artifact storage for a focused fixture,
// rather than repeatedly downloading the same unsupported payload.
type invalidPayloadError struct {
	err      error
	stage    string
	artifact *storage.Artifact
}

func (err invalidPayloadError) Error() string { return err.err.Error() }
func (err invalidPayloadError) Unwrap() error { return err.err }

func (worker Worker) retainInvalidPayload(stage string, body []byte, cause error) error {
	diagnostic, err := json.Marshal(struct {
		Stage string `json:"stage"`
		Body  []byte `json:"body_base64"`
	}{stage, body})
	if err != nil {
		return invalidPayloadError{stage: stage, err: fmt.Errorf("invalid %s payload; diagnostic encoding failed: %w", stage, err)}
	}
	artifact, created, err := worker.Artifacts.WriteWithStatus(diagnostic)
	if err != nil {
		if worker.Metrics != nil {
			worker.Metrics.RecordArtifactFailure("write")
		}
		return invalidPayloadError{stage: stage, err: fmt.Errorf("invalid %s payload: %v; diagnostic storage failed: %w", stage, cause, err)}
	}
	if created && worker.Metrics != nil {
		worker.Metrics.RecordArtifact(artifact.SizeBytes)
	}
	return invalidPayloadError{stage: stage, artifact: &artifact, err: fmt.Errorf("invalid %s payload; artifact=%s sha256=%s: %w",
		stage, artifact.RelativePath, artifact.SHA256, cause)}
}
