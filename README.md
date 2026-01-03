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
- **Sticky Recovery**: モード振動（ループ）を防ぐ強力なリカバリ機構
- **Hybrid Ricci Flow**: Sinkhorn/Formanによる効率的な曲率計算
- **REST API & gRPC**: 外部システムとの統合インターフェース
- **リアルタイムチャット**: WebSocketベースのP2Pメッセージング
- **可視化フロントエンド**: Poincaré円盤上でのネットワーク可視化

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
├── api.rs              # REST API（Axum）
├── grpc.rs             # gRPCサービス（Tonic）
├── chat.rs             # リアルタイムチャット
├── network.rs          # ネットワーク層
├── tls.rs              # TLS暗号化
├── audit.rs            # 監査ログ
└── bin/
    ├── simulator.rs              # 統合シミュレーター
    ├── scalability_experiments.rs # スケーラビリティ実験
    ├── topology_experiments.rs    # トポロジ実験
    └── baseline_comparison.rs     # ベースライン比較

frontend/                # React + TypeScript フロントエンド
├── src/
│   ├── components/
│   │   ├── PoincareDisk.tsx      # Poincaré円盤可視化
│   │   ├── ChatApp.tsx           # チャットUI
│   │   └── NodeInspectionPanel.tsx
│   └── hooks/
│       ├── useWebSocket.ts       # WebSocket接続
│       └── useRoutingAnimation.ts # ルーティングアニメーション

tests/                   # 統合テスト・プロパティテスト
├── property_tests.rs
├── api_integration_tests.rs
├── grpc_integration_tests.rs
└── tls_encryption_tests.rs

benches/                 # ベンチマーク
├── routing_latency.rs
├── coordinate_updates.rs
└── api_throughput.rs

docs/                    # ドキュメント
├── wire_protocol.md
└── audit_logging.md
```

## 実験結果サマリー

### ルーティング性能

| 埋め込み手法 | 成功率 | 平均ホップ数 |
|-------------|-------|------------|
| BFS埋め込み（旧実装） | 37.6% | - |
| PIE貪欲埋め込み | 76.2% | - |
| **Sticky Recovery** | **100.0%** | **43.14** |

### スケーラビリティ

| ノード数 | 成功率 | 平均ホップ | スループット |
|---------|-------|----------|-------------|
| 100 | 100% | 43.1 | 2,500 msg/s |
| 500 | 100% | 89.2 | 1,800 msg/s |
| 1000 | 100% | 142.5 | 1,200 msg/s |

### ベースライン比較

| アルゴリズム | 成功率 | 平均ホップ |
|-------------|-------|----------|
| DRFE-R (Gravity-Pressure) | 100% | 43.1 |
| Greedy Hyperbolic | 76.2% | 38.5 |
| Dijkstra (最短経路) | 100% | 3.2 |

## ビルド方法

### 前提条件

- Rust 1.70以上
- Node.js 18以上（フロントエンド用）
- Protocol Buffers（gRPC用）

### バックエンドビルド

```bash
cargo build --release
cargo test
```

### フロントエンドビルド

```bash
cd frontend
npm install
npm run build
```

### シミュレーター実行

```bash
# 基本実行
cargo run --release --bin simulator

# オプション付き
cargo run --release --bin simulator -- \
  -n 100 --tests 500 --ttl 50 --seed 42

# Ricci Flow最適化を有効化
cargo run --release --bin simulator -- -o --ricci-iter 20
```

### ベンチマーク実行

```bash
cargo bench
```

## API

### REST API

```bash
# ノード一覧取得
GET /api/nodes

# ルーティング実行
POST /api/route
{"source": 0, "destination": 10}

# 座標更新
PUT /api/nodes/{id}/coordinates
{"x": 0.5, "y": 0.3}
```

### gRPC

```protobuf
service RoutingService {
  rpc Route(RouteRequest) returns (RouteResponse);
  rpc GetNode(NodeRequest) returns (NodeResponse);
  rpc StreamUpdates(Empty) returns (stream CoordinateUpdate);
}
```

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
