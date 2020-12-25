use super::*;

const ERR_WRONG_USER_LOCK_HASH_SIZE: i8 = 5;
const ERR_WRONG_ORDER_DATA_SIZE: i8 = 6;
const ERR_ORDER_PRICE_IS_ZERO: i8 = 7;
const ERR_UNKNOWN_ORDER_TYPE: i8 = 8;
const ERR_UNEXPECTED_ORDER_VERSION: i8 = 9;
const ERR_UNKNOWN_OUTPUT_LOCK: i8 = 10;
const ERR_OUTPUT_TYPE_HASH_CHANGED: i8 = 11;
const ERR_OUTPUT_PRICE_CHANGED: i8 = 12;
const ERR_OUTPUT_ORDER_TYPE_CHANGED: i8 = 13;
const ERR_OUTPUT_DATA_SIZE_CHANGED: i8 = 14;
const ERR_ORDER_AMOUNT_IS_ZERO: i8 = 15;
const ERR_OUTPUT_NOT_A_SUDT_CELL: i8 = 16;
const ERR_OUTPUT_NOT_A_FREE_CELL: i8 = 17;
const ERR_BUY_CKB_PAY_ZERO_SUDT_AMOUNT: i8 = 18;
const ERR_OUTPUT_SUDT_AMOUNT_IS_ZERO: i8 = 19;
const ERR_OUTPUT_BURN_SUDT_AMOUNT: i8 = 20;
const ERR_NEGATIVE_SUDT_DIFFERENCE: i8 = 21;
const ERR_NEGATIVE_CAPACITY_DIFFERENCE: i8 = 22;
const ERR_PRICE_MISMATCH: i8 = 23;
const ERR_ORDER_STILL_MATCHABLE: i8 = 24;

#[test]
fn test_sell_ckb_complete_to_free_cell_since_we_cant_sell_even_one_ckb() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(181, 8)           // 181 ckb
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount(1)                // 1 smallest decimal sudt
        .price(28, 8)                   // 28_00_000_000
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Since price is bigger than 27 ckb, 181 ckb(order size) - 154 ckb(sudt size)
    // = 27 ckb.
    let output = OrderOutput::new_free(FreeCell::new_with_dec(181, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_complete_sell_ckb_order_since_we_cant_sell_more_price_exponent_is_negative() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity(554_00_000_004)       // 554_00_000_004
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(80, 8)        // 80 sudt
        .price(50, -1)                  // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Sold 400 ckb, got 79.76 sudt, remain 0.24 sudt, require at least 1.2 ckb, but
    // we only have 4 shannons can be sold.
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(154_00_000_004, 0, 79_76, 6));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_complete_sell_ckb_order_since_we_cant_sell_more_price_exponent_is_positive() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity(554_00_000_004)       // 554_00_000_004
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(80, 8)        // 80 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Sold 400 ckb, got 79.76 sudt, remain 0.24 sudt, require at least 1.2 ckb,
    // but we only have 4 shannons can be sold.
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(154_00_000_004, 0, 79_76, 6));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_complete_buy_ckb_order_since_we_cant_buy_more() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(154, 8)           // 154 ckb
        .sudt_amount_dec(100, 8)        // 100 sudt
        .order_amount_dec(50, 8)        // 50 ckb
        .price(5, -1)                   // 0.5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // Sold 100 sudt, got 49.85 ckb, remain 0.15 ckb, require at least 0.3 sudt,
    // but we have no more sudt to sell.
    let output = OrderOutput::new_free(FreeCell::new_with_dec(203_85, 6));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_ckb_sudt_two_orders_one_partial_filled_and_one_completed() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(1000, 8)      // 1000 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // output1 capacity = 2000 - 750 * (1 + 0.003) = 1247.75
    // output2 capacity = 800 + 750 = 1550
    let output0 = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::new_order(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_ckb_sudt_completed_matched_order_pair() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(50, 8)         // 50 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(52, -1)                  // 5.2
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));
    let output1 = OrderOutput::new_sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1], vec![output0, output1]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

// TODO: random
#[test]
fn test_ckb_sudt_two_completed_matched_order_pairs() {
    let input0 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount(0)                 // 0 sudt
        .order_amount_dec(150, 8)       // 150 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input1 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(800, 8)           // 800 ckb
        .sudt_amount_dec(500, 8)        // 500 sudt
        .order_amount_dec(750, 8)       // 750 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let input2 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(400, 8)           // 400 ckb
        .sudt_amount(0)                 // 0 sudt
        .order_amount_dec(50, 8)        // 50 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let input3 = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(400, 8)           // 400 ckb
        .sudt_amount_dec(100, 8)        // 100 sudt
        .order_amount_dec(200, 8)       // 200 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output0 = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 150, 8));
    let output1 = OrderOutput::new_sudt(SudtCell::new_with_dec(1550, 8, 349_55, 6));
    let output2 = OrderOutput::new_sudt(SudtCell::new_with_dec(199_40, 6, 50, 8));
    let output3 = OrderOutput::new_sudt(SudtCell::new_with_dec(600, 8, 59_88, 6));

    let (mut context, tx) = build_test_context(vec![input0, input1, input2, input3], vec![
        output0, output1, output2, output3,
    ]);
    let tx = context.complete_tx(tx);

    context
        .verify_tx(&tx, MAX_CYCLES)
        .expect("pass verification");
}

#[test]
fn test_err_wrong_user_lock_hash_size() {
    let input = {
        let cell = OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build();

        OrderInput::Order {
            cell_deps: None,
            cell,
            custom_lock_args: Some(Bytes::from_static(b"wrong hash size")),
            witness: None,
        }
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_WRONG_USER_LOCK_HASH_SIZE, 0));
}

#[test]
fn test_err_wrong_order_data_size() {
    let input = OrderInput::Order {
        cell_deps:        None,
        cell:             OrderCell::new_unchecked(1000, Bytes::new()), // Error: empty data size
        custom_lock_args: None,
        witness:          None,
    };

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1020, 8, 0, 0));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_WRONG_ORDER_DATA_SIZE, 0));
}

#[test]
fn test_err_order_price_is_zero() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)
        .sudt_amount_dec(50, 8)
        .order_amount_dec(150, 8)
        .price(0, 0)                    // Error: order price is zero
        .order_type(OrderType::SellCKB)
        .build(),
    );

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_ORDER_PRICE_IS_ZERO, 0));
}

#[test]
fn test_err_unknown_order_type() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)
        .sudt_amount_dec(50, 8)
        .order_amount_dec(150, 8)
        .price(5, 0)
        .order_type_unchecked(111)      // Error: unknown order type
        .build(),
    );

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_UNKNOWN_ORDER_TYPE, 0));
}

#[test]
fn test_err_unexpected_version() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)
        .sudt_amount_dec(50, 8)
        .order_amount_dec(150, 8)
        .price(5, 0)
        .order_type(OrderType::SellCKB)
        .version(100)                   // Error: unexpected version
        .build(),
    );

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1247_75, 6, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_UNEXPECTED_ORDER_VERSION, 0));
}

#[test]
fn test_err_unknown_output_lock() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .version(1)
            .build(),
    );

    // Error: pass custom lock args to create different lock
    let output = {
        let o = OrderOutput::new_free(FreeCell::new_with_dec(2000, 8));
        o.custom_lock_args(Bytes::new())
    };

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_UNKNOWN_OUTPUT_LOCK, 0));
}

#[test]
fn test_err_partial_filled_type_hash_changed() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(800, 8)
            .sudt_amount_dec(500, 8)
            .order_amount_dec(1000, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let output = OrderOutput::new_order(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );
    // Error: pass custom type args to change cell type hash
    let type_changed_output = output.custom_type_args(Bytes::from_static(b"changed"));

    let (mut context, tx) = build_test_context(vec![input], vec![type_changed_output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_TYPE_HASH_CHANGED, 0));
}

#[test]
fn test_err_partial_filled_price_changed() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(800, 8)
            .sudt_amount_dec(500, 8)
            .order_amount_dec(1000, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let output = OrderOutput::new_order(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(1, 0)                    // Error: price changed
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_PRICE_CHANGED, 0));
}

#[test]
fn test_err_partial_filled_order_type_changed() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(800, 8)
            .sudt_amount_dec(500, 8)
            .order_amount_dec(1000, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let output = OrderOutput::new_order(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(250, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB) // Error: order type changed
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_ORDER_TYPE_CHANGED, 0));
}

#[test]
fn test_err_partial_filled_data_size_changed() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(800, 8)
            .sudt_amount_dec(500, 8)
            .order_amount_dec(1000, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    // Error: pass zero bytes to change data size
    let output = OrderOutput::new_order(OrderCell::new_unchecked(1000, Bytes::new()));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_DATA_SIZE_CHANGED, 0));
}

#[test]
fn test_err_partial_filled_order_amount_is_zero() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(800, 8)
            .sudt_amount_dec(500, 8)
            .order_amount_dec(1000, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let output = OrderOutput::new_order(
        OrderCell::builder()
            .capacity_dec(1550, 8)
            .sudt_amount_dec(34955, 6)
            .order_amount_dec(0, 0)     // Error: 0 order amount
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_ORDER_AMOUNT_IS_ZERO, 0));
}

#[test]
fn test_err_type_hash_changed_output_from_completed_sell_ckb_order() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    let output = OrderOutput::new_sudt(SudtCell::new_unchecked(2000, Bytes::new()));
    // Error: pass custom type args to change cell type hash
    let type_changed_output = output.custom_type_args(Bytes::from_static(b"changed_type"));

    let (mut context, tx) = build_test_context(vec![input], vec![type_changed_output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_TYPE_HASH_CHANGED, 0));
}

#[test]
fn test_err_not_a_sudt_cell_output_from_completed_sell_ckb_order() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    // Error: pass zero bytes so that output cell data size is smaller than 16
    let output = OrderOutput::new_sudt(SudtCell::new_unchecked(2000, Bytes::new()));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_NOT_A_SUDT_CELL, 0));
}

#[test]
fn test_err_output_burn_sudt_amount_from_sell_ckb_order() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(181, 8)           // 181 ckb
        .sudt_amount_dec(10, 8)         // 10 sudt
        .order_amount(1)                // 1 smallest decimal sudt
        .price(28, 8)                   // 28_00_000_000
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Error: output burn 10 sudt
    let output = OrderOutput::new_free(FreeCell::new_with_dec(181, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_BURN_SUDT_AMOUNT, 0));
}

#[test]
fn test_err_not_a_sudt_cell_output_from_completed_buy_ckb_order() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    // Error: pass zero bytes so that output cell data size is smaller than 16
    let output = OrderOutput::new_sudt(SudtCell::new_unchecked(2000, Bytes::new()));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_NOT_A_SUDT_CELL, 0));
}

#[test]
fn test_err_not_a_free_cell_output_from_completed_buy_ckb_order() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    // Error: pass somethings, so that free data size isn't 0
    let output = OrderOutput::new_free(FreeCell::new_unchecked(
        2000,
        Bytes::from_static(b"some data"),
    ));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_NOT_A_FREE_CELL, 0));
}

#[test]
fn test_err_buy_ckb_order_sudt_amount_is_zero() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)
        .sudt_amount_dec(0, 0)          // Error: 0 sudt amount
        .order_amount_dec(150, 8)
        .price(5, 0)
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(2000, 8, 150, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_BUY_CKB_PAY_ZERO_SUDT_AMOUNT, 0));
}

#[test]
fn test_err_sell_ckb_output_sudt_amount_is_zero() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(0, 0)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    // Error: output sudt amount is zero
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(2000, 8, 0, 0));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_OUTPUT_SUDT_AMOUNT_IS_ZERO, 0));
}

#[test]
fn test_err_sell_ckb_negative_sudt_difference_output_amount_is_smaller_than_input() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(8, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    // Error: output sudt amount is smaller than inputs'
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(2000, 8, 2, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_NEGATIVE_SUDT_DIFFERENCE, 0));
}

#[test]
fn test_err_buy_ckb_negative_sudt_difference_output_amount_is_bigger_than_input() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(8, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    // Error: output sudt amount is bigger than inputs'
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(2000, 8, 10, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_NEGATIVE_SUDT_DIFFERENCE, 0));
}

#[test]
fn test_err_buy_ckb_negative_capacity_difference_input_capacity_is_bigger_than_output() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::BuyCKB)
            .build(),
    );

    // Error: input capacity is bigger than outputs
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1000, 8, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_NEGATIVE_CAPACITY_DIFFERENCE, 0));
}

#[test]
fn test_err_sell_ckb_negative_capacity_difference_output_capacity_is_bigger_than_input() {
    let input = OrderInput::new_order(
        OrderCell::builder()
            .capacity_dec(2000, 8)
            .sudt_amount_dec(50, 8)
            .order_amount_dec(150, 8)
            .price(5, 0)
            .order_type(OrderType::SellCKB)
            .build(),
    );

    // Error: output capacity is bigger than input
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(2247_75, 6, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_NEGATIVE_CAPACITY_DIFFERENCE, 0));
}

#[test]
fn test_err_sell_ckb_price_mismatch_price_exponent_is_negative() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(250, 8)       // 250 sudt
        .price(5, -1)                   // 0.5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Error: paid 1000 ckb, bought 200 sudt
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1000, 8, 200, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_PRICE_MISMATCH, 0));
}

#[test]
fn test_err_sell_ckb_price_mismatch_price_exponent_is_positive() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(2000, 8)          // 2000 ckb
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(250, 8)       // 250 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Error: paid 1000 ckb, bought 100 sudt
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(1000, 8, 100, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_PRICE_MISMATCH, 0));
}

#[test]
fn test_err_buy_ckb_price_mismatch_price_exponent_is_negative() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(200, 8)           // 200 ckb
        .sudt_amount_dec(2000, 8)       // 2000 sudt
        .order_amount_dec(500, 8)       // 500 ckb
        .price(5, -1)                   // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // Error: paid 1000 sudt, bought 300 ckb
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(500, 8, 1000, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_PRICE_MISMATCH, 0));
}

#[test]
fn test_err_buy_ckb_price_mismatch_price_exponent_is_positive() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(200, 8)           // 200 ckb
        .sudt_amount_dec(2000, 8)       // 2000 sudt
        .order_amount_dec(500, 8)       // 500 ckb
        .price(5, 0)                    // 5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // Error: paid 1000 sudt, bought 300 ckb
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(500, 8, 1000, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_PRICE_MISMATCH, 0));
}

#[test]
fn test_err_sell_ckb_order_still_matchable_price_exponent_is_negative() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(555, 8)           // 555 ckb
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(80, 8)        // 80 sudt
        .price(50, -1)                  // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Sold 400 ckb, got 79.76 sudt, remain 0.24 sudt.
    // Error: we can still sell 1_00_000_000 shannons to buy  20_000_000 sudt.
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(155, 8, 79_76, 6));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_ORDER_STILL_MATCHABLE, 0));
}

#[test]
fn test_err_sell_ckb_order_still_matchable_price_exponent_is_postive() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(555, 8)           // 555 ckb
        .sudt_amount_dec(0, 0)          // 0 sudt
        .order_amount_dec(80, 8)        // 80 sudt
        .price(5, 0)                    // 5
        .order_type(OrderType::SellCKB)
        .build(),
    );

    // Sold 400 ckb, got 79.76 sudt, remain 0.24 sudt.
    // Error: we can still sell 1_00_000_000 shannons to buy  20_000_000 sudt.
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(155, 8, 79_76, 6));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_ORDER_STILL_MATCHABLE, 0));
}

#[test]
fn test_err_buy_ckb_order_still_matchable() {
    let input = OrderInput::new_order(
        OrderCell::builder()
        .capacity_dec(200, 8)           // 200 ckb
        .sudt_amount_dec(200, 8)        // 200 sudt
        .order_amount_dec(100, 8)       // 100 ckb
        .price(5, -1)                   // 0.5
        .order_type(OrderType::BuyCKB)
        .build(),
    );

    // Error: paid 100 sudt, bought 49.85 ckb, we can paid 100 sudt to bought 49.85 ckb
    let output = OrderOutput::new_sudt(SudtCell::new_with_dec(249_85, 6, 100, 8));

    let (mut context, tx) = build_test_context(vec![input], vec![output]);
    let tx = context.complete_tx(tx);

    let err = context.verify_tx(&tx, MAX_CYCLES).unwrap_err();
    assert_error_eq!(err, tx_error(ERR_ORDER_STILL_MATCHABLE, 0));
}
