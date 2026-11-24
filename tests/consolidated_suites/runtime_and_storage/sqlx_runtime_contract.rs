use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::types::Json;
use sqlx::{FromRow, Postgres, Sqlite, SqlitePool};
use time::OffsetDateTime;

#[derive(Debug, FromRow, PartialEq)]
struct RuntimeRow {
    enabled: bool,
    small: i32,
    large: i64,
    text: String,
    moment: OffsetDateTime,
    document: Json<Value>,
    list: Json<Vec<String>>,
    maybe_float: Option<f64>,
    maybe_small: Option<i32>,
    maybe_large: Option<i64>,
    maybe_text: Option<String>,
    maybe_moment: Option<OffsetDateTime>,
    maybe_document: Option<Json<Value>>,
    maybe_list: Option<Json<Vec<String>>>,
}

async fn pool() -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap()
}

fn require_pg<T: for<'r> FromRow<'r, sqlx::postgres::PgRow>>() {}
fn require_sqlite<T: for<'r> FromRow<'r, sqlx::sqlite::SqliteRow>>() {}

#[test]
fn official_derive_supports_both_database_types() {
    require_pg::<RuntimeRow>();
    require_sqlite::<RuntimeRow>();
    let _: Option<sqlx::Transaction<'static, Postgres>> = None;
    let _: Option<sqlx::Transaction<'static, Sqlite>> = None;
    let _: sqlx::QueryBuilder<Postgres> = sqlx::QueryBuilder::new("SELECT ");
    let _: sqlx::QueryBuilder<Sqlite> = sqlx::QueryBuilder::new("SELECT ");
}

#[tokio::test]
async fn named_mapping_preserves_all_current_field_types_and_extra_columns() {
    let pool = pool().await;
    let moment = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let row: RuntimeRow = sqlx::query_as(
        "SELECT 'unused' AS extra, ? AS maybe_list, ? AS maybe_document, \
         ? AS maybe_moment, 'optional' AS maybe_text, 9223372036854775807 AS maybe_large, \
         -2147483648 AS maybe_small, 1.25 AS maybe_float, ? AS list, ? AS document, \
         ? AS moment, 'hello' AS text, -9223372036854775808 AS large, \
         2147483647 AS small, 1 AS enabled",
    )
    .bind(Json(vec!["a".to_string()]))
    .bind(Json(json!({"ok": true})))
    .bind(moment)
    .bind(Json(vec!["b".to_string()]))
    .bind(Json(json!({"count": 2})))
    .bind(moment)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        RuntimeRow {
            enabled: true,
            small: i32::MAX,
            large: i64::MIN,
            text: "hello".into(),
            moment,
            document: Json(json!({"count": 2})),
            list: Json(vec!["b".into()]),
            maybe_float: Some(1.25),
            maybe_small: Some(i32::MIN),
            maybe_large: Some(i64::MAX),
            maybe_text: Some("optional".into()),
            maybe_moment: Some(moment),
            maybe_document: Some(Json(json!({"ok": true}))),
            maybe_list: Some(Json(vec!["a".into()])),
        }
    );
    pool.close().await;
}

#[tokio::test]
async fn null_option_fields_remain_none() {
    #[derive(Debug, FromRow, PartialEq)]
    struct Nullable {
        number: Option<i64>,
        moment: Option<OffsetDateTime>,
        document: Option<Json<Value>>,
    }
    let pool = pool().await;
    let row: Nullable = sqlx::query_as("SELECT NULL AS number, NULL AS moment, NULL AS document")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row,
        Nullable {
            number: None,
            moment: None,
            document: None
        }
    );
    pool.close().await;
}

#[tokio::test]
async fn official_attribute_and_raw_identifier_mapping_remain_available() {
    #[derive(Debug, FromRow, PartialEq, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Attributes {
        stored_value: i64,
        r#type: String,
        #[sqlx(rename = "wire_value")]
        renamed: i64,
        #[sqlx(default)]
        absent: Option<String>,
    }
    let pool = pool().await;
    let row: Attributes =
        sqlx::query_as("SELECT 7 AS wire_value, 'name' AS type, 5 AS stored_value")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        row,
        Attributes {
            stored_value: 5,
            r#type: "name".into(),
            renamed: 7,
            absent: None,
        }
    );
    pool.close().await;
}

#[tokio::test]
// These rows are intentionally used only on decoding-error paths.
#[allow(dead_code)]
async fn official_error_behavior_snapshot() {
    #[derive(Debug, FromRow)]
    struct Number {
        value: i64,
    }
    #[derive(Debug, FromRow)]
    struct Text {
        value: String,
    }
    #[derive(Debug, FromRow)]
    struct Optional {
        value: Option<i64>,
    }
    #[derive(Debug, FromRow)]
    struct Document {
        value: Json<Value>,
    }
    #[derive(Debug, FromRow)]
    struct Moment {
        value: OffsetDateTime,
    }
    #[derive(Debug, FromRow)]
    struct Ordered {
        first: i64,
        second: i64,
    }

    fn snapshot(error: sqlx::Error, expected: &str) -> Value {
        match &error {
            sqlx::Error::ColumnNotFound(column) if expected == "missing" => {
                assert_eq!(column, "value");
                json!({"kind": "ColumnNotFound", "column": column, "display": error.to_string()})
            }
            sqlx::Error::ColumnDecode { index, source } if expected != "missing" => {
                assert!(index.contains(expected));
                json!({"kind": "ColumnDecode", "index": index, "source": source.to_string(), "display": error.to_string()})
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    let pool = pool().await;
    let missing = sqlx::query_as::<Sqlite, Optional>("SELECT 1 AS other")
        .fetch_one(&pool)
        .await
        .unwrap_err();
    let mismatch = sqlx::query_as::<Sqlite, Number>("SELECT 'not-an-integer' AS value")
        .fetch_one(&pool)
        .await
        .unwrap_err();
    let null_text = sqlx::query_as::<Sqlite, Text>("SELECT NULL AS value")
        .fetch_one(&pool)
        .await
        .unwrap();
    // SQLx SQLite's official String decoder maps NULL to empty text, not an error.
    assert_eq!(null_text.value, "");
    let json_error = sqlx::query_as::<Sqlite, Document>("SELECT '{bad' AS value")
        .fetch_one(&pool)
        .await
        .unwrap_err();
    let time_error = sqlx::query_as::<Sqlite, Moment>("SELECT 'not-a-time' AS value")
        .fetch_one(&pool)
        .await
        .unwrap_err();
    let first_error = sqlx::query_as::<Sqlite, Ordered>("SELECT 'x' AS first, 'y' AS second")
        .fetch_one(&pool)
        .await
        .unwrap_err();
    let snapshot = json!({
        "missing_optional": snapshot(missing, "missing"),
        "type_mismatch": snapshot(mismatch, "value"),
        "null_required": {"kind": "Ok", "value": null_text.value},
        "invalid_json": snapshot(json_error, "value"),
        "invalid_time": snapshot(time_error, "value"),
        "first_error": snapshot(first_error, "first"),
    });
    // 期望取自独立官方 umbrella 对照，不从本次运行动态生成。
    let expected: Value =
        serde_json::from_str(include_str!("fixtures/sqlx_error_snapshot.json")).unwrap();
    assert_eq!(snapshot, expected);
    println!("SQLX_ERROR_SNAPSHOT {snapshot}");
    pool.close().await;
}
