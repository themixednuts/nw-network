use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use nw_network::{
    ReplicatedState,
    hub::{DynFragment, MarshalContext, SequenceNumber},
    serialize::{Marshaler, ReadBuffer, ReplicatedFieldHandler, WriteBuffer},
};
use std::hint::black_box;

const FIELD_COUNT: usize = 16;

#[derive(Debug, Default, ReplicatedState)]
struct PackedState {
    base: nw_network::hub::ReplicatedState,
    f00: ReplicatedFieldHandler<u32>,
    f01: ReplicatedFieldHandler<u32>,
    f02: ReplicatedFieldHandler<u32>,
    f03: ReplicatedFieldHandler<u32>,
    f04: ReplicatedFieldHandler<u32>,
    f05: ReplicatedFieldHandler<u32>,
    f06: ReplicatedFieldHandler<u32>,
    f07: ReplicatedFieldHandler<u32>,
    f08: ReplicatedFieldHandler<u32>,
    f09: ReplicatedFieldHandler<u32>,
    f10: ReplicatedFieldHandler<u32>,
    f11: ReplicatedFieldHandler<u32>,
    f12: ReplicatedFieldHandler<u32>,
    f13: ReplicatedFieldHandler<u32>,
    f14: ReplicatedFieldHandler<u32>,
    f15: ReplicatedFieldHandler<u32>,
}

#[derive(Clone, Copy, Debug)]
struct NaiveOptionalState {
    fields: [Option<u32>; FIELD_COUNT],
}

#[derive(Clone, Copy, Debug)]
struct NaiveFullState {
    fields: [u32; FIELD_COUNT],
}

impl NaiveOptionalState {
    fn marshal(&self, wb: &mut WriteBuffer) {
        for value in self.fields {
            value.is_some().marshal(wb);
            if let Some(value) = value {
                value.marshal(wb);
            }
        }
    }
}

impl NaiveFullState {
    fn marshal(&self, wb: &mut WriteBuffer) {
        for value in self.fields {
            value.marshal(wb);
        }
    }
}

fn context() -> MarshalContext<'static> {
    MarshalContext {
        baseline_seq: SequenceNumber::Invalid,
        filter_target: None,
        group_baselines: None,
    }
}

fn packed_sparse() -> PackedState {
    let mut state = PackedState::default();
    state.f00.set_value(10);
    state.f07.set_value(70);
    state.f14.set_value(140);
    state
}

fn packed_dense() -> PackedState {
    let mut state = PackedState::default();
    state.f00.set_value(0);
    state.f01.set_value(1);
    state.f02.set_value(2);
    state.f03.set_value(3);
    state.f04.set_value(4);
    state.f05.set_value(5);
    state.f06.set_value(6);
    state.f07.set_value(7);
    state.f08.set_value(8);
    state.f09.set_value(9);
    state.f10.set_value(10);
    state.f11.set_value(11);
    state.f12.set_value(12);
    state.f13.set_value(13);
    state.f14.set_value(14);
    state.f15.set_value(15);
    state
}

fn naive_optional_sparse() -> NaiveOptionalState {
    let mut fields = [None; FIELD_COUNT];
    fields[0] = Some(10);
    fields[7] = Some(70);
    fields[14] = Some(140);
    NaiveOptionalState { fields }
}

fn naive_optional_dense() -> NaiveOptionalState {
    let mut fields = [None; FIELD_COUNT];
    for (idx, value) in fields.iter_mut().enumerate() {
        *value = Some(u32::try_from(idx).expect("FIELD_COUNT fits in u32"));
    }
    NaiveOptionalState { fields }
}

fn naive_full_dense() -> NaiveFullState {
    let mut fields = [0; FIELD_COUNT];
    for (idx, value) in fields.iter_mut().enumerate() {
        *value = u32::try_from(idx).expect("FIELD_COUNT fits in u32");
    }
    NaiveFullState { fields }
}

fn write_packed(state: &PackedState, wb: &mut WriteBuffer) -> usize {
    wb.clear();
    DynFragment::marshal_contents_with(state, &context(), wb);
    wb.len()
}

fn write_naive_optional(state: &NaiveOptionalState, wb: &mut WriteBuffer) -> usize {
    wb.clear();
    state.marshal(wb);
    wb.len()
}

fn write_naive_full(state: &NaiveFullState, wb: &mut WriteBuffer) -> usize {
    wb.clear();
    state.marshal(wb);
    wb.len()
}

fn read_packed(bytes: &[u8]) -> PackedState {
    let mut rb = ReadBuffer::carrier(bytes);
    let mut state = PackedState::default();
    DynFragment::unmarshal_contents(&mut state, &mut rb).expect("packed state decodes");
    state
}

fn bench_sparse(c: &mut Criterion) {
    let packed = packed_sparse();
    let naive = naive_optional_sparse();
    let mut wb = WriteBuffer::carrier();
    let packed_len = write_packed(&packed, &mut wb);
    let naive_len = write_naive_optional(&naive, &mut wb);

    let mut group = c.benchmark_group("replicated_state_sparse");
    group.throughput(Throughput::Bytes(packed_len as u64));
    group.bench_function("packed", |b| {
        let mut wb = WriteBuffer::carrier_with_capacity(64);
        b.iter(|| black_box(write_packed(black_box(&packed), black_box(&mut wb))));
    });
    group.throughput(Throughput::Bytes(naive_len as u64));
    group.bench_function("naive_optional", |b| {
        let mut wb = WriteBuffer::carrier_with_capacity(96);
        b.iter(|| black_box(write_naive_optional(black_box(&naive), black_box(&mut wb))));
    });
    group.finish();
}

fn bench_dense(c: &mut Criterion) {
    let packed = packed_dense();
    let naive_optional = naive_optional_dense();
    let naive_full = naive_full_dense();
    let mut wb = WriteBuffer::carrier();
    let packed_len = write_packed(&packed, &mut wb);
    let naive_optional_len = write_naive_optional(&naive_optional, &mut wb);
    let naive_full_len = write_naive_full(&naive_full, &mut wb);

    let mut group = c.benchmark_group("replicated_state_dense");
    group.throughput(Throughput::Bytes(packed_len as u64));
    group.bench_function("packed", |b| {
        let mut wb = WriteBuffer::carrier_with_capacity(96);
        b.iter(|| black_box(write_packed(black_box(&packed), black_box(&mut wb))));
    });
    group.throughput(Throughput::Bytes(naive_optional_len as u64));
    group.bench_function("naive_optional", |b| {
        let mut wb = WriteBuffer::carrier_with_capacity(128);
        b.iter(|| {
            black_box(write_naive_optional(
                black_box(&naive_optional),
                black_box(&mut wb),
            ))
        });
    });
    group.throughput(Throughput::Bytes(naive_full_len as u64));
    group.bench_function("naive_full", |b| {
        let mut wb = WriteBuffer::carrier_with_capacity(96);
        b.iter(|| black_box(write_naive_full(black_box(&naive_full), black_box(&mut wb))));
    });
    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let packed = packed_sparse();
    let mut wb = WriteBuffer::carrier_with_capacity(64);
    write_packed(&packed, &mut wb);
    let bytes = wb.into_vec();

    let mut group = c.benchmark_group("replicated_state_decode");
    group.throughput(Throughput::Bytes(bytes.len() as u64));
    group.bench_function("packed_sparse", |b| {
        b.iter_batched(
            || bytes.as_slice(),
            |bytes| black_box(read_packed(black_box(bytes))),
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, bench_sparse, bench_dense, bench_decode);
criterion_main!(benches);
