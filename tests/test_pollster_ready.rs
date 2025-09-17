#[test]
fn test_pollster_with_ready() {
    let ready_fut = std::future::ready(42);
    let result = pollster::block_on(ready_fut);
    assert_eq!(result, 42);
}
