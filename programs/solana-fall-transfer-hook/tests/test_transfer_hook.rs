#[allow(dead_code)]
mod helpers;

use {
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

use helpers::{
    build_transfer_with_hook_ix, create_ata, initialize_rate_limit, mint_tokens, setup,
    setup_mint_and_extra_metas,
};

#[test]
fn test_transfer_hook() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    let mint_amount = 1_000_000u64;
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        mint_amount,
    );

    let transfer_ix = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        100,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg =
        Message::new_with_blockhash(&[transfer_ix], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Transfer with hook failed: {:?}",
        res.err()
    );
}

#[test]
fn test_transfer_hook_rate_limit_exceeded() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    let recipient = Keypair::new();
    svm.airdrop(&recipient.pubkey(), 1_000_000_000).unwrap();

    let source_ata = create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());
    let dest_ata = create_ata(
        &mut svm,
        &payer,
        &recipient.pubkey(),
        &mint.pubkey(),
    );

    // Mint more than the rate limit so we have enough tokens.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata,
        2_000_000,
    );

    // First transfer: exactly at the limit - should succeed.
    let ix1 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Transfer at limit should succeed: {:?}",
        res.err()
    );

    // Second transfer: 1 more - should fail with RateLimitExceeded.
    let ix2 = build_transfer_with_hook_ix(
        &source_ata,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[ix2], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_err(),
        "Transfer exceeding rate limit should fail"
    );
}

#[test]
fn test_rate_limit_is_per_user() {
    let (mut svm, payer, program_id) = setup();
    let mint = Keypair::new();

    // Set up the mint, payer 1's rate limit, and the extra account meta list.
    setup_mint_and_extra_metas(&mut svm, &payer, &mint, &program_id);

    // Create and fund the second wallet.
    let payer2 = Keypair::new();
    svm.airdrop(&payer2.pubkey(), 1_000_000_000).unwrap();

    // Give payer 2 its own rate-limit account for this mint.
    initialize_rate_limit(&mut svm, &payer2, &mint, &program_id);

    // Both users will send to the same recipient.
    let recipient = Keypair::new();

    let source_ata1 =
        create_ata(&mut svm, &payer, &payer.pubkey(), &mint.pubkey());

    let source_ata2 =
        create_ata(&mut svm, &payer, &payer2.pubkey(), &mint.pubkey());

    let dest_ata =
        create_ata(&mut svm, &payer, &recipient.pubkey(), &mint.pubkey());

    // Give each sender 1,000,000 base units.
    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata1,
        1_000_000,
    );

    mint_tokens(
        &mut svm,
        &payer,
        &mint.pubkey(),
        &source_ata2,
        1_000_000,
    );

    // Payer 1 sends their full 1,000,000 limit.
    let ix1 = build_transfer_with_hook_ix(
        &source_ata1,
        &dest_ata,
        &mint.pubkey(),
        &payer.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg =
        Message::new_with_blockhash(&[ix1], Some(&payer.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "First user's transfer should succeed: {:?}",
        res.err()
    );

    // Payer 2 also sends their full 1,000,000 limit in the same hour.
    let ix2 = build_transfer_with_hook_ix(
        &source_ata2,
        &dest_ata,
        &mint.pubkey(),
        &payer2.pubkey(),
        &program_id,
        1_000_000,
        9,
    );

    let blockhash = svm.latest_blockhash();
    let msg =
        Message::new_with_blockhash(&[ix2], Some(&payer2.pubkey()), &blockhash);
    let tx =
        VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer2]).unwrap();

    let res = svm.send_transaction(tx);

    assert!(
        res.is_ok(),
        "Second user's transfer should also succeed: {:?}",
        res.err()
    );
}