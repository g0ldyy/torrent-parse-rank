use std::hint::black_box;
use std::sync::LazyLock;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rtn_core::{check_fetch, get_rank, parse};
use serde_json::{Value, json};

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

static SETTINGS: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "require": [],
        "exclude": [],
        "preferred": [],
        "options": {
            "remove_all_trash": true,
            "remove_unknown_languages": false,
            "allow_english_in_languages": true,
            "remove_adult_content": true
        },
        "languages": {
            "required": [],
            "allowed": [],
            "exclude": []
        },
        "resolutions": {
            "r2160p": true,
            "r1080p": true,
            "r720p": true,
            "r480p": true,
            "r360p": true,
            "unknown": true
        },
        "custom_ranks": {}
    })
});

static RANK_MODEL: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "av1": 500,
        "avc": 500,
        "bluray": 100,
        "hdtv": -5000,
        "hevc": 500,
        "remux": 10000,
        "web": 100,
        "webdl": 200,
        "webmux": -10000,
        "xvid": -10000,
        "webrip": -1000,
        "hdr": 2000,
        "hdr10plus": 2100,
        "dolby_vision": 3000,
        "bit_10": 100,
        "aac": 100,
        "atmos": 1000,
        "dolby_digital_plus": 150,
        "dts_lossy": 100,
        "dts_lossless": 2000,
        "truehd": 2000,
        "surround": 100,
        "three_d": -10000,
        "proper": 20,
        "repack": 20,
        "site": -10000,
        "upscaled": -10000,
        "cam": -10000,
        "clean_audio": -10000,
        "r5": -10000,
        "screener": -10000,
        "telecine": -10000,
        "telesync": -10000
    })
});

fn rtn_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("rtn_core");

    group.bench_function("parse", |b| {
        let mut idx = 0usize;
        b.iter(|| {
            let title = TITLES[idx % TITLES.len()];
            idx += 1;
            let parsed = parse(black_box(title), black_box(false)).expect("parse should pass");
            black_box(parsed);
        });
    });

    group.bench_function("parse_fetch_rank", |b| {
        let settings = &*SETTINGS;
        let rank_model = &*RANK_MODEL;
        let mut idx = 0usize;
        b.iter(|| {
            let title = TITLES[idx % TITLES.len()];
            idx += 1;
            let parsed = parse(black_box(title), black_box(false)).expect("parse should pass");
            let (fetch, failed) =
                check_fetch(&parsed, settings, black_box(true)).expect("fetch check should pass");
            let rank = get_rank(&parsed, settings, rank_model).expect("rank should pass");
            black_box((fetch, failed.len(), rank));
        });
    });

    group.bench_function("batch_128_parse_fetch_rank", |b| {
        let settings = &*SETTINGS;
        let rank_model = &*RANK_MODEL;
        b.iter_batched(
            || {
                (0..128)
                    .map(|i| TITLES[i % TITLES.len()])
                    .collect::<Vec<_>>()
            },
            |batch| {
                let mut aggregate_rank = 0i64;
                let mut aggregate_fetch = 0usize;
                for title in batch {
                    let parsed = parse(title, false).expect("parse should pass");
                    let (fetch, _) =
                        check_fetch(&parsed, settings, true).expect("fetch should pass");
                    let rank = get_rank(&parsed, settings, rank_model).expect("rank should pass");
                    aggregate_rank += rank;
                    if fetch {
                        aggregate_fetch += 1;
                    }
                }
                black_box((aggregate_rank, aggregate_fetch));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, rtn_benches);
criterion_main!(benches);
