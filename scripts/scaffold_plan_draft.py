"""生成计划草稿的脚手架脚本。"""
import json
from datetime import datetime
from pathlib import Path

def scaffold_plan():
    """生成基础计划草稿。"""
    plan = {
        "title": "分片 GGUF 文件支持计划",
        "created": datetime.now().isoformat(),
        "version": "0.1.0",
        "status": "draft",
        "scope": "支持分片 GGUF 模型文件的加载、显示与启动",
    }
    return plan

if __name__ == "__main__":
    plan = scaffold_plan()
    print(json.dumps(plan, indent=2, ensure_ascii=False))
