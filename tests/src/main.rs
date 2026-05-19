use kapi_tests::{
    object_crud, optimistic_concurrency, schema_deletion, schema_validation, watch_events,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("RUST_LOG"))
        .init();

    let mut passed = 0u32;
    let mut failed = 0u32;

    macro_rules! run_test {
        ($name:expr, $test:expr) => {{
            print!("  {} ... ", $name);
            match $test {
                Ok(()) => {
                    println!("ok");
                    passed += 1;
                }
                Err(e) => {
                    println!("FAILED");
                    println!("    {e}");
                    failed += 1;
                }
            }
        }};
    }

    println!("running integration tests");
    println!();

    println!("test object_crud");
    run_test!("create_schema_then_object", object_crud::test_create_schema_then_object().await);
    run_test!("full_crud_flow", object_crud::test_full_crud_flow().await);
    run_test!("list_single_page", object_crud::test_list_single_page().await);
    run_test!("list_two_pages", object_crud::test_list_two_pages().await);
    run_test!("list_resume_position", object_crud::test_list_resume_position().await);
    run_test!("list_exhausted", object_crud::test_list_exhausted().await);

    println!();
    println!("test watch_events");
    run_test!("watch_schema_added", watch_events::test_watch_schema_added().await);
    run_test!("watch_object_events", watch_events::test_watch_object_events().await);

    println!();
    println!("test schema_deletion");
    run_test!("delete_schema_no_objects", schema_deletion::test_delete_schema_no_objects().await);
    run_test!("delete_schema_with_objects", schema_deletion::test_delete_schema_with_objects().await);

    println!();
    println!("test schema_validation");
    run_test!("valid_schema_accepted", schema_validation::test_valid_schema_accepted().await);
    run_test!("invalid_json_schema_rejected", schema_validation::test_invalid_json_schema_rejected().await);
    run_test!("missing_required_fields_rejected", schema_validation::test_missing_required_fields_rejected().await);

    println!();
    println!("test optimistic_concurrency");
    run_test!("update_correct_rv", optimistic_concurrency::test_update_correct_rv().await);
    run_test!("update_wrong_rv", optimistic_concurrency::test_update_wrong_rv().await);

    println!();
    println!("test result: {passed} passed, {failed} failed");

    if failed > 0 {
        std::process::exit(1);
    }
}
