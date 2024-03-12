//! Benchmarking of `DomainMap` operations

mod data;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use data::REAL_DOMAINS;
use links_domainmap::{Domain, DomainMap};

pub fn domainmap_lookup_exists(c: &mut Criterion) {
	let mut group = c.benchmark_group("DomainMap::get(existing domain)");
	for amount in [
		1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 112, 128,
	] {
		group.throughput(Throughput::Elements(amount as u64));
		group.bench_with_input(
			BenchmarkId::from_parameter(amount),
			&(
				DomainMap::<usize>::from_iter(
					REAL_DOMAINS[..amount]
						.iter()
						.map(|d| (Domain::presented(d).unwrap(), amount)),
				),
				Domain::reference(REAL_DOMAINS[amount / 2]).unwrap(),
			),
			|b, (map, domain)| {
				b.iter(|| black_box(map.get(black_box(domain))));
			},
		);
	}
	group.finish();
}

pub fn domainmap_lookup_not_exists(c: &mut Criterion) {
	let mut group = c.benchmark_group("DomainMap::get(nonexisting domain)");
	for amount in [
		1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32, 40, 48, 56, 64, 80, 96, 112, 128,
	] {
		group.throughput(Throughput::Elements(amount as u64));
		group.bench_with_input(
			BenchmarkId::from_parameter(amount),
			&(
				DomainMap::<usize>::from_iter(
					REAL_DOMAINS[..amount]
						.iter()
						.map(|d| (Domain::presented(d).unwrap(), amount)),
				),
				Domain::reference(REAL_DOMAINS.last().unwrap()).unwrap(),
			),
			|b, (map, domain)| {
				b.iter(|| black_box(map.get(black_box(domain))));
			},
		);
	}
	group.finish();
}

criterion_group!(
	benches,
	domainmap_lookup_exists,
	domainmap_lookup_not_exists
);
criterion_main!(benches);
