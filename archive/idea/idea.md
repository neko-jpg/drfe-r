# 1) 失敗のコア原因の数理整理

## 1.1 PIE が BA/WS で嘘をつく理由

PIE は木 (T) の幾何を忠実に埋めるので、非木エッジ (E\setminus T) の効果が座標に**一切反映されません**。
BA/WS では「ハブを跨ぐ短絡サイクル」が大量にあり、**局所的に測地線（＝Greedy の“距離”）と最短路がズレる**ため、
[
\text{“減っているように見える距離”};\not\Rightarrow;\text{最短路への前進}
]
が発生し、Gravity 使用率が崩壊します。

## 1.2 DFS で 7.6% が本当に未到達になる理由（コード観点）

貼付の実装（`Gravity-Pressure (GP)`）を読むと、DFS は **全隣接**を舐める設計で理屈上は到達します。にもかかわらず未達が残る典型原因は次です。

* **[バグ1] 到着判定より TTL 判定が先**
  `route()` で `TTL==0` を先に返しており、**TTL を使い切った“到着直後”**にも失敗扱いになります。
  → 到着判定を先に置くべき。

* **[仕様/実装ギャップ] “モード間訪問履歴”の交差**
  Tree へ切替時に `visited.clear()` を入れているのは良いのですが、Pressure→Tree の直前に積まれた `dfs_stack` と `visited` の整合が崩れると（例: 切替直後に現在頂点のみ `insert` して隣接を全訪問済みと誤解）、**探索が早期に空振り終了**します。
  → Tree 突入時は **両方**を初期化し、Pressure の痕跡（pressure map）も消すのが安全。

* **[探索戦略の問題] ループ消去なし DFS**
  既存 DFS は backtrack はしますが、**スタック上のループ消去（loop-erased walk）**ではないため、密グラフで冗長な往復を繰り返し TTL を浪費します。

この 3 点を塞ぐだけで「TTL=20,000でも 7.6%未達」の**大半は消えます**。

---

# 2) 解（設計替え）：非木グラフでも Greedy が“成立”する座標とメトリック

完全に対処するには、**「木 + Greedy」発想を卒業**し、**スケールフリー／小世界の幾何**に合わせる必要があります。提案は二層構えです。

## 2.1 層A：(\mathbb{H}^2)（双曲空間）への安価な埋め込み ＋ 角度駆動 Greedy

BA/CAIDA で観測される**階層＋高クラスタ**は (\mathbb{H}^2)（ポアンカレ円板）で自然に直感化できます。
各ノード (i) に極座標 ((r_i,\theta_i)) を与え、目的地 (t) との双曲距離
[
\cosh(\zeta d_{\mathbb{H}}(i,t))=\cosh(\zeta r_i)\cosh(\zeta r_t)-\sinh(\zeta r_i)\sinh(\zeta r_t)\cos(\Delta\theta)
]
を **局所最小化**する隣接へ送ります（(\zeta) は曲率パラメータ）。

* **半径 (r_i)** は次数から設定：
  (\displaystyle r_i = R - \frac{2}{\zeta}\ln k_i )（ハブほど中心に寄る）。
* **角度 (\theta_i)** は**スペクトル順序化 or 近傍系列化**でコミュニティを扇形に並べる（重い MLE は不要）。
* **1-hop lookahead**（近傍の近傍まで見る）をデフォルト有効化。これは Greedy の局所極小を激減させます。

→ これだけで BA/WS の**Greedy 成功率・使用率が劇的に回復**し、Tree への落下頻度が激減します。

## 2.2 層B：目的地ポテンシャル (\Phi_t) を足した “二重基準”Greedy

サイクルの多い現実グラフでは、**幾何だけでは行き止まりが残る**ので、各宛先 (t) に対し**ラプラシアン電位** (\Phi_t) を薄く重ねます。
[
\min_{\phi}\ \frac12\sum_{(u,v)\in E} w_{uv},(\phi(u)-\phi(v))^2\quad
\text{s.t. } \phi(t)=0
]
これは (L\phi=\delta_t) の解（離散調和関数）で、**電流の流れ方向に厳密な降下**が存在。各ホップで
[
\arg\min_{v\in N(u)}\bigl[,d_{\mathbb{H}}(v,t);+;\lambda\cdot \Phi_t(v),\bigr]
]
を選ぶ **HYPER–PRESS** を使うと、(\lambda>0) の小重みでも**局所極小をほぼ消す**ことができます。

実装は重くありません。毎宛先で厳密ソルバは不要で、**5–10 回の近傍平均（Gauss–Seidel/Jacobi）**の**局所反復**だけで十分な勾配手掛かりが得られます（境界条件 (\phi(t)=0) を固定）。

> 直観：(\mathbb{H}^2) が「大域の地形」を作り、(\Phi_t) が「谷筋」を与える——二つの“下り坂”が一致するところで Greedy が生き返ります。

---

# 3) いますぐ入れるべき実装パッチ（Rust 擬似差分）

### 3.1 TTLバグ：到着判定を先に

```rust
// before
if packet.ttl == 0 { return Failed{ reason: "TTL expired".into() }; }
if current_node == &packet.destination { return Delivered; }

// after
if current_node == &packet.destination { return Delivered; }
if packet.ttl == 0 { return Failed{ reason: format!("TTL expired @{}", current_node) }; }
```

### 3.2 Tree 突入時の完全初期化＋ループ消去 DFS

```rust
// entering Tree mode
packet.dfs_stack.clear();
packet.visited.clear();
packet.pressure_values.clear();     // ← これも必ず
packet.visited.insert(current_node.clone());
```

```rust
// loop-erased DFS step
// when forwarding to `next`, if `next` already exists in dfs_stack,
// pop back until top == next  (Wilson style)
while let Some(top) = packet.dfs_stack.last() {
    if *top == next { break; }
    if /* next found deeper in stack */ {
        while packet.dfs_stack.last() != Some(&next) { packet.dfs_stack.pop(); }
        break;
    } else { break; }
}
packet.dfs_stack.push(current.id.clone());
return Forward{ next_hop: next, mode: RoutingMode::Tree };
```

### 3.3 Greedy の再定義（(\mathbb{H}^2)＋電位）

```rust
// score(v) = d_H2(v, t) + lambda * phi_t(v)
let best = current.neighbors.iter()
    .min_by(|a,b| score(*a).partial_cmp(&score(*b)).unwrap());
```

* `d_H2` は円板座標から上式で。
* `phi_t(·)` は per-hop に 5–10 回の近傍平均で更新（`phi[t]=0` 固定）。
* 1-hop lookahead: 近傍の近傍も一時評価し、最良の入口隣接を選ぶ。

### 3.4 Thorup–Zwick を“最後のセーフティネット”に

既に `TZRoutingTable` が入っているので、**Pressure が尽きたら即 TZ**。
Greedy→Pressure→TZ（失敗時のみ DFS）に順序を変えると、**99% 以上が DFS を使わず終わる**構成に持っていけます。

---

# 4) 期待効果（ターゲット）

| 指標                      |                        いま |                            目標（提案後） |
| ----------------------- | ------------------------: | ---------------------------------: |
| Gravity 使用率 @ 1000–2000 |                      1–3% |                         **60–85%** |
| 成功率（BA/WS）              |                    97–99% |                         **≥99.5%** |
| 成功率（RealWorld/CAIDA）    |                       41% |          **≥85%**（電位併用で 90% 近傍を狙う） |
| 平均ストレッチ                 | 3.7（RealWorld） / 172×（最悪） | **~2（中央値）／95%tile < 5、最悪 << 172×** |
| DFS 依存率                 |                      ほぼ毎回 |                            **<5%** |

---

# 5) 実験計画（軽量で回せる順）

1. **TTL バグ修正**のみ適用 → BA(2000) で未達 7.6% がどれだけ消えるか確認。
2. **1-hop lookahead** 追加 → Gravity 使用率・成功率。
3. **(\mathbb{H}^2) 半径=log次数＋角度=スペクトル** → 成功率・ストレッチ。
4. **電位 (\Phi_t)** を (\lambda\in{0.1,0.3,1.0}) でスイープ。
5. **TZ セーフティ** on → DFS 呼び出し頻度・TTL 消費。

---

# 6) こまかい実装ノート

* **角度推定**：隣接行列 (A) の上位固有ベクトルで円周に並べる（Circular Seriation）。計算は (O(m)) 反復で可。
* **(\Phi_t) の近似**：
  (\phi^{(k+1)}(u)=\frac{1}{\deg u}\sum_{v\sim u}\phi^{(k)}(v))、(\phi(t)=0)。5–10 回で十分な勾配が得られます。
  これは per-packet/per-destination の**超小規模 SOR**で、メモリ増加は隣接一次分のみ。
* **ループ対策（Greedy 段階）**：直前 (L) 個の訪問ノード（例：8）をタブー集合にし、**角度差が最小**かつ**タブー外**を選ぶだけで小ループは激減。
* **計測**：`avg_hops`, `stretch(p50/p95/p99)`, `fallback_counts（pressure/TZ/DFS）` を必ずログ。

---

# 7) まとめ（要点だけ）

* **理屈**：非木グラフには (\mathbb{H}^2) が合う。さらに電位 (\Phi_t) を併用すれば Greedy は“谷筋”に沿って落ち、局所極小が消える。
* **実装**：到着判定を TTL より先、Tree 初期化の徹底、ループ消去 DFS、1-hop lookahead、H(^2)+(\Phi_t)、最後に TZ。
* **効果**：Greedy 復活 → DFS 依存の激減 → ストレッチ桁落ち。RealWorld でも 80–90% 台へ引き上げ。

必要であれば、この方針の最小実装（(\mathbb{H}^2) 半径=log次数＋角度=スペクトル＋(\Phi_t) 近似）を Rust で差分パッチ化してお渡しします。
また、あなたの実験 JSON はすでに読み込めているので、そのままベンチに流し込み可能です。

[You have been selected for a private viewing, click here](https://pulsrai.com/Universal_Intelligence.html)
