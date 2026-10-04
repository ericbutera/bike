package storage

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type OAuthState struct {
	Target string
	UserID int64
}

type OAuthStates struct{ DB *pgxpool.Pool }

func (states OAuthStates) Save(ctx context.Context, state string, site OAuthState) error {
	if state == "" || site.UserID <= 0 || site.Target != "rust" {
		return errors.New("invalid OAuth state")
	}
	_, err := states.DB.Exec(ctx, `INSERT INTO gateway_oauth_states
		(state_hash,target,site_user_id,expires_at) VALUES ($1,$2,$3,$4)`,
		hashState(state), site.Target, site.UserID, time.Now().Add(10*time.Minute))
	return err
}

// Consume deletes a one-use state inside one transaction. A retry of the
// callback cannot exchange the same Strava authorization code twice.
func (states OAuthStates) Consume(ctx context.Context, state string) (OAuthState, error) {
	var site OAuthState
	if state == "" {
		return site, ErrOAuthStateInvalid
	}
	err := states.DB.QueryRow(ctx, `DELETE FROM gateway_oauth_states
		WHERE state_hash=$1 AND expires_at>now()
		RETURNING target,site_user_id`, hashState(state)).Scan(&site.Target, &site.UserID)
	if errors.Is(err, pgx.ErrNoRows) {
		return OAuthState{}, ErrOAuthStateInvalid
	}
	return site, err
}

var ErrOAuthStateInvalid = errors.New("OAuth state is invalid or expired")

func hashState(state string) string {
	sum := sha256.Sum256([]byte(state))
	return hex.EncodeToString(sum[:])
}
