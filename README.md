# DRFE-R: Distributed Ricci Flow Embedding with Rendezvous Mechanism

[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**双曲空間を用いた分散ルーティングプロトコルの実装**

## 概要

DRFE-Rは、動的ネットワークにおける分散ルーティングプロトコルです。従来のRicci Flowを用いた双曲埋め込みアプローチの課題（Coordinate-ID Paradox）を解決し、理論的な到達保証と現実的なスケーラビリティを両立させます。

### 主な特徴

- **Poincaré円盤モデル**: 双曲距離計算による効率的なルーティング
- **二重座標系**: トポロジ非依存のAnchor座標とトポロジ依存のRouting座標
- **PIE貪欲埋め込み**: Polar Increasing-angle Embeddingによる座標割り当て
- **Gravity-Pressureルーティング**: 局所最小を回避する理論的到達保証
- **Hybrid Ricci Flow**: Sinkhorn/Formanによる効率的な曲率計算

## プロジェクト構成

```
src/
├── lib.rs              # Poincaré円盤モデル（双曲距離計算）
├── coordinates.rs      # 二重座標系（Anchor/Routing）
├── greedy_embedding.rs # PIE貪欲埋め込みアルゴリズム
├── routing.rs          # Gravity-Pressureルーティング
├── ricci.rs            # Hybrid Ricci Flow（Sinkhorn/Forman）
├── rendezvous.rs       # 2フェーズランデブー機構
├── stability.rs        # 近接正則化・ドリフト追跡
└── bin/
    └── simulator.rs    # 統合シミュレーター（CLI付き）
```

## 現在の状況

### 完了項目

| モジュール | 説明 | テスト数 |
|-----------|------|---------|
| `lib.rs` | Poincaré円盤モデル（双曲距離計算） | 6 |
| `coordinates.rs` | 二重座標系（Anchor/Routing） | 5 |
| `routing.rs` | GPルーティング（Gravity/Pressure modes） | 3 |
| `ricci.rs` | Hybrid Ricci Flow（Sinkhorn/Forman） | 4 |
| `rendezvous.rs` | 2フェーズランデブー機構 | 3 |
| `stability.rs` | 近接正則化・ドリフト追跡 | 3 |
| `greedy_embedding.rs` | PIE貪欲埋め込み | 4 |
| `simulator.rs` | 統合シミュレーター（CLI付き） | - |

**全28ユニットテスト成功**

### シミュレーション結果

100ノード Barabási-Albert ネットワーク、500ルーティングテスト：

| 埋め込み手法 | 成功率 | 備考 |
|-------------|-------|------|
| BFS埋め込み（旧実装） | 37.6% | 座標がトポロジを反映せず |
| PIE貪欲埋め込み | **76.2%** | スパニングツリーベース |

### 既知の課題

1. **非ツリーエッジの影響**: PIE埋め込みはスパニングツリー上で100%成功を保証しますが、Barabási-Albertネットワークの非ツリーエッジがGreedy Forwardingで局所最小を引き起こす可能性があります

2. **目標達成**: 95%+の成功率を達成するには、追加の最適化が必要です

## ビルド方法

### 前提条件

- Rust 1.70以上
- Cargo

### コンパイル

```bash
cargo build --release
```

### テスト実行

```bash
cargo test
```

### シミュレーター実行

```bash
# 基本実行
cargo run --release --bin simulator

# オプション付き
cargo run --release --bin simulator -- \
  -n 100        \  # ノード数
  --tests 500   \  # ルーティングテスト数
  --ttl 50      \  # 最大TTL
  --seed 42        # 乱数シード

# Ricci Flow最適化を有効化
cargo run --release --bin simulator -- -o --ricci-iter 20
```

### CLI オプション

| オプション | 説明 | デフォルト |
|-----------|------|-----------|
| `-n, --nodes` | ノード数 | 100 |
| `-t, --topology` | トポロジ (random/barabasi-albert) | barabasi-albert |
| `--tests` | ルーティングテスト数 | 100 |
| `--ttl` | 最大TTL | 50 |
| `--seed` | 乱数シード | 42 |
| `-o, --optimize` | Ricci Flow最適化を有効化 | false |
| `--ricci-iter` | Ricci Flow反復回数 | 10 |

## 理論的背景

詳細は [theory.md](theory.md) を参照してください。

### Coordinate-ID Paradox

従来のアプローチでは、ノードIDから直接座標を導出することは、座標がトポロジに依存するため不可能でした。DRFE-Rは以下の二重座標系でこれを解決します：

1. **Anchor座標**: IDからハッシュで決定論的に導出（トポロジ非依存）
2. **Routing座標**: Ricci Flowで動的に更新（トポロジ依存）

### Gravity-Pressure ルーティング

- **Gravity Mode**: 双曲距離を最小化する隣接ノードへ転送
- **Pressure Mode**: 局所最小に陥った場合、圧力を用いて脱出

## 参考文献

- [Kleinberg 2007] "Geographic Routing Using Hyperbolic Space"
- [Cvetkovski-Crovella 2009] "Hyperbolic Embedding and Routing for Dynamic Graphs"
- [Ollivier 2009] "Ricci curvature of Markov chains on metric spaces"

## ライセンス

MIT License
