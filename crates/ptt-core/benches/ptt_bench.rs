use std::hint::black_box;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ptt_core::{parse_many, parse_title};

const TITLES: [&str; 20] = [
    "The.Walking.Dead.S05E03.720p.WEB-DL.x264-ASAP[ettv]",
    "Game.of.Thrones.S01.1080p.BluRay.x265.10bit.AAC.7.1",
    "Spider-Man.No.Way.Home.2021.2160p.BluRay.REMUX.HEVC.TrueHD.7.1.Atmos",
    "Oppenheimer.2023.BluRay.1080p.DTS-HD.MA.5.1.AVC.REMUX",
    "The.Last.of.Us.S01E01.2160p.WEB-DL.DDP5.1.Atmos.HDR.HEVC",
    "Dune.Part.Two.2024.1080p.WEB-DL.DDP5.1.Atmos.H.264",
    "Breaking.Bad.S03E10.720p.BluRay.x264",
    "Attack.on.Titan.S04E01.1080p.WEB-DL.x265.10bit.Multi-Subs",
    "Mission.Impossible.Pentalogy.1996-2015.1080p.BluRay.x264.AAC.5.1",
    "The.Matrix.1999.2160p.UHD.BluRay.x265.HDR10.TrueHD.7.1",
    "Interstellar.2014.1080p.BluRay.x264.DTS",
    "The.Office.US.S02E12.1080p.NF.WEB-DL.DDP5.1.x264",
    "John.Wick.Chapter.4.2023.2160p.WEB-DL.DV.HDR10Plus.HEVC",
    "Avatar.The.Way.of.Water.2022.2160p.BluRay.REMUX.HEVC.Atmos",
    "The.Boys.S04E03.1080p.AMZN.WEB-DL.DDP5.1.H.264",
    "Severance.S01E02.1080p.ATVP.WEB-DL.DDP5.1.Atmos.H.264",
    "Andor.S01E11.2160p.DSNP.WEB-DL.DDP5.1.Atmos.DV.HEVC",
    "Shogun.2024.S01E05.1080p.HULU.WEB-DL.DDP5.1.H.264",
    "The.Batman.2022.2160p.BluRay.x265.DTS-HD.MA.5.1.HDR",
    "Top.Gun.Maverick.2022.1080p.BluRay.x264.DTS",
];

fn ptt_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("ptt_core");

    group.bench_function("parse_title_translate_false", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            let title = TITLES[idx % TITLES.len()];
            idx += 1;
            let parsed =
                parse_title(black_box(title), black_box(false)).expect("parse should pass");
            black_box(parsed);
        });
    });

    group.bench_function("parse_title_translate_true", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            let title = TITLES[idx % TITLES.len()];
            idx += 1;
            let parsed = parse_title(black_box(title), black_box(true)).expect("parse should pass");
            black_box(parsed);
        });
    });

    group.bench_function("parse_many_128_translate_false", |b| {
        b.iter_batched(
            || {
                (0..128)
                    .map(|i| TITLES[i % TITLES.len()])
                    .collect::<Vec<_>>()
            },
            |batch| {
                let parsed = parse_many(batch.iter().copied(), black_box(false))
                    .expect("batch parse should pass");
                black_box(parsed);
            },
            BatchSize::SmallInput,
        );
    });

    group.bench_function("parse_many_128_translate_true", |b| {
        b.iter_batched(
            || {
                (0..128)
                    .map(|i| TITLES[i % TITLES.len()])
                    .collect::<Vec<_>>()
            },
            |batch| {
                let parsed = parse_many(batch.iter().copied(), black_box(true))
                    .expect("batch parse should pass");
                black_box(parsed);
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, ptt_benches);
criterion_main!(benches);
