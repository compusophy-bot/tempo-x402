import urllib.request
import json

def summarize_soul():
    try:
        with urllib.request.urlopen("http://localhost:8080/soul") as response:
            data = json.loads(response.read().decode())
    except Exception as e:
        return {"error": str(e)}

    # Top-level priorities from goals
    goals = data.get("goals", [])
    sorted_goals = sorted(goals, key=lambda x: x.get("priority", 0), reverse=True)
    top_priorities = [g.get("description") for g in sorted_goals]

    # Recent learnings from recent_thoughts
    recent_thoughts = data.get("recent_thoughts", [])
    learning_types = ["Observation", "Reflection", "MemoryConsolidation"]
    recent_learnings = [
        t.get("content") for t in recent_thoughts 
        if t.get("type") in learning_types
    ]

    summary = {
        "top_level_priorities": top_priorities,
        "recent_learnings": recent_learnings
    }
    return summary

if __name__ == "__main__":
    print(json.dumps(summarize_soul(), indent=2))
