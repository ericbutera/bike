//! Opt-in verification of the PostgreSQL provisioning lock on a disposable database.
use bike_core::entities::synthetic_scenarios;
use sea_orm::sea_query::{Alias, Func, Query};
use sea_orm::{Database, TransactionTrait};

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a disposable PostgreSQL database"]
async fn provisioning_lock_serializes_transactions_and_releases_on_commit() {
    use sea_orm::ConnectionTrait;
    let url = std::env::var("BIKE_TEST_DATABASE_URL").expect("disposable database URL");
    let first = Database::connect(&url)
        .await
        .unwrap()
        .begin()
        .await
        .unwrap();
    let second = Database::connect(&url)
        .await
        .unwrap()
        .begin()
        .await
        .unwrap();
    synthetic_scenarios::Model::lock_provisioning(&first)
        .await
        .unwrap();
    let query = Query::select()
        .expr_as(
            Func::cust(Alias::new("pg_try_advisory_xact_lock")).arg(743918260_i64),
            Alias::new("acquired"),
        )
        .to_owned();
    let acquired = second.query_one(&query).await.unwrap().unwrap();
    assert!(!acquired.try_get::<bool>("", "acquired").unwrap());
    first.commit().await.unwrap();
    let acquired = second.query_one(&query).await.unwrap().unwrap();
    assert!(acquired.try_get::<bool>("", "acquired").unwrap());
    second.rollback().await.unwrap();
}
