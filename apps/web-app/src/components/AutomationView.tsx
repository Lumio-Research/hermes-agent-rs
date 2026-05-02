import { useEffect, useState } from "react";
import { Plus } from "lucide-react";
import * as api from "../api";
import type { AutomationTask } from "../types";

export function AutomationView() {
  const [tasks, setTasks] = useState<AutomationTask[]>([]);

  useEffect(() => {
    api.getAutomationTasks().then(setTasks);
  }, []);

  // Group tasks by category
  const grouped = tasks.reduce<Record<string, AutomationTask[]>>((acc, task) => {
    if (!acc[task.category]) acc[task.category] = [];
    acc[task.category].push(task);
    return acc;
  }, {});

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-2xl font-semibold text-text-primary">自动化</h1>
            <p className="text-sm text-text-muted mt-1">
              通过设置定期聊天，实现工作自动化。
              <a href="#" className="text-accent hover:text-accent-hover ml-1">
                了解更多
              </a>
            </p>
          </div>
          <button className="flex items-center gap-2 px-4 py-2 rounded-lg bg-bg-tertiary border border-border-primary text-sm text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors">
            <Plus size={16} />
            新建自动化
          </button>
        </div>

        {/* Task categories */}
        {Object.entries(grouped).map(([category, categoryTasks]) => (
          <div key={category} className="mb-8">
            <h2 className="text-base font-medium text-text-primary mb-3">
              {category}
            </h2>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {categoryTasks.map((task) => (
                <TaskCard key={task.id} task={task} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function TaskCard({ task }: { task: AutomationTask }) {
  return (
    <button className="flex items-start gap-3 p-4 rounded-xl bg-bg-card border border-border-primary hover:bg-bg-card-hover hover:border-border-secondary transition-colors text-left group">
      <span className="text-lg shrink-0 mt-0.5">{task.icon}</span>
      <p className="text-sm text-text-secondary group-hover:text-text-primary leading-relaxed">
        {task.title}
      </p>
    </button>
  );
}
