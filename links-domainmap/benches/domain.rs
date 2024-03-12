//! Benchmarking of `Domain` operations

mod data;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use data::{DOMAIN_PRESENTED, DOMAIN_REFERENCE, REAL_DOMAINS};
use links_domainmap::Domain;

pub fn domain_presented_real(c: &mut Criterion) {
	let mut group = c.benchmark_group("Domain::presented(real domains)");
	let amount = REAL_DOMAINS.len();

	group.throughput(Throughput::Elements(amount as u64));
	group.bench_with_input(
		BenchmarkId::from_parameter(amount),
		&REAL_DOMAINS,
		|b, &input| {
			b.iter(|| {
				for domain in input {
					let _ = black_box(Domain::presented(black_box(domain)));
				}
			})
		},
	);

	group.finish();
}

pub fn domain_reference_real(c: &mut Criterion) {
	let mut group = c.benchmark_group("Domain::reference(real domains)");
	let amount = REAL_DOMAINS.len();

	group.throughput(Throughput::Elements(amount as u64));
	group.bench_with_input(
		BenchmarkId::from_parameter(amount),
		&REAL_DOMAINS,
		|b, &input| {
			b.iter(|| {
				for domain in input {
					let _ = black_box(Domain::reference(black_box(domain)));
				}
			})
		},
	);

	group.finish();
}

pub fn domain_presented_example(c: &mut Criterion) {
	let mut group = c.benchmark_group("Domain::presented(sample domains)");
	let amount = DOMAIN_PRESENTED.len();

	group.throughput(Throughput::Elements(amount as u64));
	group.bench_with_input(
		BenchmarkId::from_parameter(amount),
		&DOMAIN_PRESENTED,
		|b, &input| {
			b.iter(|| {
				for domain in input {
					let _ = black_box(Domain::presented(black_box(domain)));
				}
			})
		},
	);

	group.finish();
}

pub fn domain_reference_example(c: &mut Criterion) {
	let mut group = c.benchmark_group("Domain::reference(sample domains)");
	let amount = DOMAIN_REFERENCE.len();

	group.throughput(Throughput::Elements(amount as u64));
	group.bench_with_input(
		BenchmarkId::from_parameter(amount),
		&DOMAIN_REFERENCE,
		|b, &input| {
			b.iter(|| {
				for domain in input {
					let _ = black_box(Domain::reference(black_box(domain)));
				}
			})
		},
	);

	group.finish();
}

criterion_group!(
	benches,
	domain_presented_real,
	domain_reference_real,
	domain_presented_example,
	domain_reference_example
);
criterion_main!(benches);
