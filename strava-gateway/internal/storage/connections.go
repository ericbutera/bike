package storage

import (
	"context"
	"encoding/json"
	"errors"
	"strconv"
	"time"

	"github.com/ericbutera/bike-services/strava-gateway/internal/secret"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

type Connection struct {
	AthleteID    int64     `json:"athlete_id"`
	AccessToken  string    `json:"access_token"`
	RefreshToken string    `json:"refresh_token"`
	ExpiresAt    time.Time `json:"expires_at"`
	Scopes       []string  `json:"scopes"`
}

type Connections struct {
	DB     *pgxpool.Pool
	Cipher secret.TokenCipher
}

func (connections Connections) Upsert(ctx context.Context, item Connection) error {
	sealed, err := connections.sealTokens(item)
	if err != nil {
		return err
	}
	_, err = connections.DB.Exec(ctx, `INSERT INTO gateway_connections
		(athlete_id, token_ciphertext, expires_at, scopes)
		VALUES ($1,$2,$3,$4)
		ON CONFLICT (athlete_id) DO UPDATE SET
		 token_ciphertext=excluded.token_ciphertext, expires_at=excluded.expires_at,
		 scopes=excluded.scopes, revoked_at=NULL, updated_at=now()`,
		item.AthleteID, sealed, item.ExpiresAt, item.Scopes)
	return err
}

// ImportRust links the legacy user to the gateway. Replacing an existing token
// is only for the handoff after Rust's token-refresh worker has stopped.
func (connections Connections) ImportRust(ctx context.Context, item Connection, userID int64, replace bool) (bool, error) {
	if userID <= 0 {
		return false, errors.New("invalid Rust user ID")
	}
	sealed, err := connections.sealTokens(item)
	if err != nil {
		return false, err
	}
	tx, err := connections.DB.Begin(ctx)
	if err != nil {
		return false, err
	}
	defer tx.Rollback(ctx)
	statement := `INSERT INTO gateway_connections
		(athlete_id,token_ciphertext,expires_at,scopes) VALUES ($1,$2,$3,$4)
		ON CONFLICT (athlete_id) DO NOTHING`
	if replace {
		statement = `INSERT INTO gateway_connections
			(athlete_id,token_ciphertext,expires_at,scopes) VALUES ($1,$2,$3,$4)
			ON CONFLICT (athlete_id) DO UPDATE SET token_ciphertext=excluded.token_ciphertext,
			 expires_at=excluded.expires_at, scopes=excluded.scopes, revoked_at=NULL, updated_at=now()`
	}
	inserted, err := tx.Exec(ctx, statement, item.AthleteID, sealed, item.ExpiresAt, item.Scopes)
	if err != nil {
		return false, err
	}
	linked, err := tx.Exec(ctx, `INSERT INTO gateway_site_links
		(athlete_id,target,site_user_id) VALUES ($1,'rust',$2)
		ON CONFLICT (athlete_id,target) DO UPDATE SET enabled=true,updated_at=now()
		WHERE gateway_site_links.site_user_id=excluded.site_user_id`, item.AthleteID, userID)
	if err != nil {
		return false, err
	}
	if linked.RowsAffected() != 1 {
		return false, ErrLinkConflict
	}
	if err := tx.Commit(ctx); err != nil {
		return false, err
	}
	return inserted.RowsAffected() == 1, nil
}

func (connections Connections) sealTokens(item Connection) ([]byte, error) {
	if item.AthleteID <= 0 || item.AccessToken == "" || item.RefreshToken == "" || item.ExpiresAt.IsZero() {
		return nil, errors.New("incomplete Strava connection")
	}
	plain, err := json.Marshal(struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
	}{item.AccessToken, item.RefreshToken})
	if err != nil {
		return nil, err
	}
	sealed, err := connections.Cipher.Encrypt([]byte(strconv.FormatInt(item.AthleteID, 10)), plain)
	return sealed, err
}

func (connections Connections) Find(ctx context.Context, athleteID int64) (Connection, error) {
	var sealed []byte
	var item Connection
	item.AthleteID = athleteID
	err := connections.DB.QueryRow(ctx, `SELECT token_ciphertext, expires_at, scopes
		FROM gateway_connections WHERE athlete_id=$1 AND revoked_at IS NULL`, athleteID).
		Scan(&sealed, &item.ExpiresAt, &item.Scopes)
	if err != nil {
		return Connection{}, err
	}
	plain, err := connections.Cipher.Decrypt([]byte(strconv.FormatInt(athleteID, 10)), sealed)
	if err != nil {
		return Connection{}, err
	}
	var tokens struct {
		AccessToken  string `json:"access_token"`
		RefreshToken string `json:"refresh_token"`
	}
	if err := json.Unmarshal(plain, &tokens); err != nil {
		return Connection{}, err
	}
	item.AccessToken, item.RefreshToken = tokens.AccessToken, tokens.RefreshToken
	return item, nil
}

func (connections Connections) Revoke(ctx context.Context, athleteID int64) error {
	// Replace usable tokens before retaining the audit row.
	sealed, err := connections.Cipher.Encrypt([]byte(strconv.FormatInt(athleteID, 10)), []byte(`{}`))
	if err != nil {
		return err
	}
	_, err = connections.DB.Exec(ctx, `UPDATE gateway_connections SET token_ciphertext=$2,
		revoked_at=now(), updated_at=now() WHERE athlete_id=$1`, athleteID, sealed)
	return err
}

type SiteLink struct {
	AthleteID int64
	Target    string
	UserID    int64
}

func (connections Connections) Link(ctx context.Context, link SiteLink) error {
	if link.AthleteID <= 0 || link.UserID <= 0 || link.Target != "rust" {
		return errors.New("invalid site link")
	}
	command, err := connections.DB.Exec(ctx, `INSERT INTO gateway_site_links
		(athlete_id, target, site_user_id) VALUES ($1,$2,$3)
		ON CONFLICT (athlete_id, target) DO UPDATE SET site_user_id=excluded.site_user_id,
		enabled=true, updated_at=now()
		WHERE gateway_site_links.site_user_id=excluded.site_user_id`, link.AthleteID, link.Target, link.UserID)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return ErrLinkConflict
	}
	return nil
}

func (connections Connections) FindLink(ctx context.Context, athleteID int64, target string) (SiteLink, error) {
	var link SiteLink
	link.AthleteID, link.Target = athleteID, target
	err := connections.DB.QueryRow(ctx, `SELECT site_user_id FROM gateway_site_links
		WHERE athlete_id=$1 AND target=$2 AND enabled=true`, athleteID, target).Scan(&link.UserID)
	if errors.Is(err, pgx.ErrNoRows) {
		return SiteLink{}, ErrLinkNotFound
	}
	return link, err
}

func (connections Connections) FindLinkByUser(ctx context.Context, target string, userID int64) (SiteLink, error) {
	var link SiteLink
	link.Target, link.UserID = target, userID
	err := connections.DB.QueryRow(ctx, `SELECT athlete_id FROM gateway_site_links
		WHERE target=$1 AND site_user_id=$2 AND enabled=true`, target, userID).Scan(&link.AthleteID)
	if errors.Is(err, pgx.ErrNoRows) {
		return SiteLink{}, ErrLinkNotFound
	}
	return link, err
}

func (connections Connections) DisableLink(ctx context.Context, link SiteLink) error {
	tx, err := connections.DB.Begin(ctx)
	if err != nil {
		return err
	}
	defer tx.Rollback(ctx)
	command, err := tx.Exec(ctx, `UPDATE gateway_site_links SET enabled=false,updated_at=now()
		WHERE athlete_id=$1 AND target=$2 AND site_user_id=$3 AND enabled=true`,
		link.AthleteID, link.Target, link.UserID)
	if err != nil {
		return err
	}
	if command.RowsAffected() != 1 {
		return ErrLinkNotFound
	}
	if _, err := tx.Exec(ctx, `UPDATE gateway_sync_jobs SET status='succeeded',
		lease_until=NULL,waiting_reason=NULL,updated_at=now() WHERE athlete_id=$1 AND target=$2
		AND status IN ('queued','processing')`, link.AthleteID, link.Target); err != nil {
		return err
	}
	return tx.Commit(ctx)
}

var ErrLinkNotFound = errors.New("site link not found")
var ErrLinkConflict = errors.New("Strava athlete is linked to another Bike user")
