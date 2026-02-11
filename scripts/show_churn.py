import json
data = json.load(open("paper_data/churn/churn_robustness.json"))
print(f"{'Strategy':20s} {'Selection':10s} {'Rem%':5s} {'Success':10s} {'Stretch':10s} {'MaxStr':10s} {'TZ%':8s}")
print("-" * 70)
for run in data["runs"]:
    for r in run["results"]:
        print(f"{r['strategy']:20s} {r['selection']:10s} {r['removal_rate']:5.0f}% {r['success_rate']*100:9.1f}% {r['avg_stretch']:10.3f} {r['max_stretch']:10.1f} {r['tz_pct']:7.1f}%")
