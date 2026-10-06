package worker

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"

	"github.com/ericbutera/bike-services/strava-gateway/internal/storage"
)

type Artifacts struct{ Root string }

func (artifacts Artifacts) DiskUsage() (int64, error) {
	var total int64
	err := filepath.WalkDir(artifacts.Root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() {
			return nil
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		total += info.Size()
		return nil
	})
	if os.IsNotExist(err) {
		return 0, nil
	}
	return total, err
}

var ErrArtifactUnavailable = errors.New("strava artifact unavailable")

// ReadByHash supports operator retrieval without trusting a supplied file path.
func (artifacts Artifacts) ReadByHash(hash string) ([]byte, error) {
	decoded, err := hex.DecodeString(hash)
	if err != nil || len(decoded) != sha256.Size || hex.EncodeToString(decoded) != hash {
		return nil, errors.New("invalid artifact SHA256")
	}
	relative := filepath.Join(hash[:2], hash+".json")
	info, err := os.Stat(filepath.Join(artifacts.Root, relative))
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrArtifactUnavailable, err)
	}
	return artifacts.Read(storage.Artifact{SHA256: hash, RelativePath: relative, SizeBytes: info.Size()})
}

func (artifacts Artifacts) Write(data []byte) (storage.Artifact, error) {
	item, _, err := artifacts.WriteWithStatus(data)
	return item, err
}

func (artifacts Artifacts) WriteWithStatus(data []byte) (item storage.Artifact, created bool, err error) {
	if len(data) == 0 {
		return storage.Artifact{}, false, errors.New("empty Strava artifact")
	}
	sum := sha256.Sum256(data)
	hash := hex.EncodeToString(sum[:])
	relative := filepath.Join(hash[:2], hash+".json")
	destination := filepath.Join(artifacts.Root, relative)
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		return storage.Artifact{}, false, err
	}
	if existing, err := os.ReadFile(destination); err == nil {
		if len(existing) == len(data) {
			existingHash := sha256.Sum256(existing)
			if existingHash == sum {
				return storage.Artifact{SHA256: hash, RelativePath: relative, SizeBytes: int64(len(data))}, false, nil
			}
		}
	} else if !errors.Is(err, os.ErrNotExist) {
		return storage.Artifact{}, false, err
	}
	file, err := os.CreateTemp(filepath.Dir(destination), ".strava-*")
	if err != nil {
		return storage.Artifact{}, false, err
	}
	temp := file.Name()
	defer func() {
		if cleanupErr := os.Remove(temp); cleanupErr != nil && !errors.Is(cleanupErr, os.ErrNotExist) {
			err = errors.Join(err, cleanupErr)
		}
	}()
	if err := file.Chmod(0o600); err != nil {
		return storage.Artifact{}, false, errors.Join(err, file.Close())
	}
	if _, err := file.Write(data); err != nil {
		return storage.Artifact{}, false, errors.Join(err, file.Close())
	}
	if err := file.Sync(); err != nil {
		return storage.Artifact{}, false, errors.Join(err, file.Close())
	}
	if err := file.Close(); err != nil {
		return storage.Artifact{}, false, err
	}
	if err := os.Rename(temp, destination); err != nil {
		return storage.Artifact{}, false, err
	}
	return storage.Artifact{SHA256: hash, RelativePath: relative, SizeBytes: int64(len(data))}, true, nil
}

func (artifacts Artifacts) Read(item storage.Artifact) ([]byte, error) {
	if len(item.SHA256) != 64 || item.RelativePath != filepath.Join(item.SHA256[:2], item.SHA256+".json") {
		return nil, errors.New("invalid artifact path")
	}
	data, err := os.ReadFile(filepath.Join(artifacts.Root, item.RelativePath))
	if err != nil {
		return nil, fmt.Errorf("%w: %v", ErrArtifactUnavailable, err)
	}
	sum := sha256.Sum256(data)
	if hex.EncodeToString(sum[:]) != item.SHA256 || int64(len(data)) != item.SizeBytes {
		return nil, fmt.Errorf("%w: integrity check failed for %s", ErrArtifactUnavailable, item.SHA256)
	}
	return data, nil
}
