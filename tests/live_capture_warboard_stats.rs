//! Live-capture validation of the type-839 warboard stats wire layout.

use nw_network::generated_messages::WarboardComponentClientFacetOnUpdateWarboardStats;
use nw_network::serialize::{
    CARRIER_ENDIAN, Marshal, MarshalerError, ReadBuffer, Unmarshal, WriteBuffer,
};
use nw_network::{ActorRequestId, WarboardStatRow};

const LIVE_INITIAL: &[u8] = include_bytes!("fixtures/live/type839_warboard_stats_initial.bin");
const LIVE_LATE_MATCH: &[u8] =
    include_bytes!("fixtures/live/type839_warboard_stats_late_match.bin");
const LIVE_SMALLEST: &[u8] = include_bytes!("fixtures/live/type839_warboard_stats_smallest.bin");
const LIVE_UNMODELLED: &[u8] =
    include_bytes!("fixtures/live/type839_warboard_stats_unmodelled.bin");

fn decode(bytes: &[u8]) -> WarboardComponentClientFacetOnUpdateWarboardStats {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, bytes);
    let message = WarboardComponentClientFacetOnUpdateWarboardStats::unmarshal(&mut rb)
        .expect("warboard stats message");
    assert_eq!(rb.left(), 0, "body consumed exactly");

    let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
    message.marshal(&mut wb);
    assert_eq!(wb.into_vec(), bytes, "marshal reproduces the captured body");

    message
}

#[test]
fn live_initial_body_decodes_all_zero_rows() {
    let message = decode(LIVE_INITIAL);

    assert_eq!(message.header, ActorRequestId::new(0, 0));
    let stats = &message.stats;
    assert_eq!(stats.counter, 7);
    assert_eq!(stats.local_index, 3);
    assert_eq!(stats.local, WarboardStatRow::default());

    let block_sizes: Vec<usize> = stats
        .blocks
        .iter()
        .map(|block| block.players.len())
        .collect();
    assert_eq!(block_sizes, vec![3, 4]);

    for block in &stats.blocks {
        for player in &block.players {
            assert!(
                player.row.bits().eq([1, 2, 7, 21, 22, 23, 26, 27, 38]),
                "row {} carries the opening stat set",
                player.player_index
            );
            assert!(
                player.row.values.iter().all(|value| *value == 0),
                "row {} is all zero at match start",
                player.player_index
            );
        }
    }
}

#[test]
fn live_late_match_body_decodes_running_totals() {
    let message = decode(LIVE_LATE_MATCH);
    let stats = &message.stats;

    assert_eq!(stats.counter, 40);
    assert_eq!(stats.local_index, 3);
    assert!(stats.local.bits().eq([8, 9, 25, 28, 29]));
    assert_eq!(stats.local.stat(8), Some(65_117));
    assert_eq!(stats.local.stat(9), Some(19_542));
    assert_eq!(stats.local.stat(25), Some(2));
    assert_eq!(stats.local.stat(28), Some(8));
    assert_eq!(stats.local.stat(29), Some(1_167));

    let block_sizes: Vec<usize> = stats
        .blocks
        .iter()
        .map(|block| block.players.len())
        .collect();
    assert_eq!(block_sizes, vec![8, 7]);

    let row_17 = stats.blocks[0]
        .players
        .iter()
        .find(|player| player.player_index == 17)
        .expect("block 0 row 17");
    assert_eq!(row_17.row.stat(1), Some(4_296));
    assert_eq!(row_17.row.stat(2), Some(509_923));

    let row_8 = stats.blocks[1]
        .players
        .iter()
        .find(|player| player.player_index == 8)
        .expect("block 1 row 8");
    assert_eq!(row_8.row.stat(1), Some(13_356));
    assert_eq!(row_8.row.stat(2), Some(362_968));
    assert_eq!(row_8.row.stat(21), Some(747_441));
}

#[test]
fn live_smallest_body_decodes_empty_rows() {
    let message = decode(LIVE_SMALLEST);
    let stats = &message.stats;

    assert_eq!(stats.counter, 13);
    assert_eq!(stats.local_index, 3);
    assert_eq!(stats.local, WarboardStatRow::default());

    let block_sizes: Vec<usize> = stats
        .blocks
        .iter()
        .map(|block| block.players.len())
        .collect();
    assert_eq!(block_sizes, vec![0, 1]);

    let only = &stats.blocks[1].players[0];
    assert_eq!(only.player_index, 7);
    assert_eq!(only.row, WarboardStatRow::default());
}

#[test]
fn live_unmodelled_body_is_rejected() {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, LIVE_UNMODELLED);
    let result = WarboardComponentClientFacetOnUpdateWarboardStats::unmarshal(&mut rb);
    assert!(
        matches!(result, Err(MarshalerError::InvalidRange { .. })),
        "row shapes with mask bit 0 must not be guessed: {result:?}"
    );
}
