use kapi_tests::{
    TestApp, all_test_stores, object_crud, object_labels, optimistic_concurrency, schema_deletion,
    schema_validation, watch_events,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("RUST_LOG"))
        .init();

    let stores = all_test_stores();
    let mut overall_failed = false;

    for store in &stores {
        println!("=== {} ===", store.name);
        println!();

        macro_rules! run_test {
            ($name:expr, $test:expr) => {{
                print!("  {} ... ", $name);
                let app = TestApp::with_store((store.factory)());
                match $test(&app).await {
                    Ok(()) => println!("ok"),
                    Err(e) => {
                        println!("FAILED");
                        println!("    {e}");
                        overall_failed = true;
                    }
                }
            }};
        }

        run_test!(
            "create_schema_then_object",
            object_crud::test_create_schema_then_object
        );
        run_test!("full_crud_flow", object_crud::test_full_crud_flow);
        run_test!("list_single_page", object_crud::test_list_single_page);
        run_test!("list_two_pages", object_crud::test_list_two_pages);
        run_test!(
            "list_resume_position",
            object_crud::test_list_resume_position
        );
        run_test!("list_exhausted", object_crud::test_list_exhausted);

        println!();
        run_test!(
            "create_object_with_labels",
            object_labels::test_create_object_with_labels
        );
        run_test!(
            "create_object_without_labels",
            object_labels::test_create_object_without_labels
        );
        run_test!(
            "update_object_labels",
            object_labels::test_update_object_labels
        );
        run_test!(
            "create_schema_with_labels",
            object_labels::test_create_schema_with_labels
        );
        run_test!(
            "invalid_label_key_400",
            object_labels::test_invalid_label_key_format
        );
        run_test!(
            "invalid_label_value_400",
            object_labels::test_invalid_label_value_format
        );
        run_test!(
            "label_key_too_long_400",
            object_labels::test_label_key_exceeds_length
        );
        run_test!(
            "label_value_too_long_400",
            object_labels::test_label_value_exceeds_length
        );

        println!();
        run_test!("watch_schema_added", watch_events::test_watch_schema_added);
        run_test!(
            "watch_object_events",
            watch_events::test_watch_object_events
        );
        run_test!(
            "watch_by_name_matching",
            watch_events::test_watch_by_name_matching_events
        );
        run_test!(
            "watch_by_name_non_matching_filtered",
            watch_events::test_watch_by_name_non_matching_filtered
        );
        run_test!(
            "watch_invalid_field_selector",
            watch_events::test_watch_invalid_field_selector
        );
        run_test!(
            "field_selector_non_watch_400",
            watch_events::test_field_selector_on_non_watch_returns_400
        );
        run_test!(
            "watch_by_name_and_all_simultaneous",
            watch_events::test_watch_by_name_and_watch_all_simultaneously
        );
        run_test!(
            "watcher_cleanup_on_disconnect",
            watch_events::test_watcher_cleanup_on_client_disconnect
        );

        println!();
        run_test!(
            "delete_schema_no_objects",
            schema_deletion::test_delete_schema_no_objects
        );
        run_test!(
            "delete_schema_with_objects",
            schema_deletion::test_delete_schema_with_objects
        );

        println!();
        run_test!(
            "valid_schema_accepted",
            schema_validation::test_valid_schema_accepted
        );
        run_test!(
            "invalid_json_schema_rejected",
            schema_validation::test_invalid_json_schema_rejected
        );
        run_test!(
            "missing_required_fields_rejected",
            schema_validation::test_missing_required_fields_rejected
        );

        println!();
        run_test!(
            "update_correct_rv",
            optimistic_concurrency::test_update_correct_rv
        );
        run_test!(
            "update_wrong_rv",
            optimistic_concurrency::test_update_wrong_rv
        );

        println!();

        if overall_failed {
            println!("FAILED: {} store", store.name);
            std::process::exit(1);
        }

        println!("passed: {} store", store.name);
        println!();
    }

    println!("all stores passed");
}
