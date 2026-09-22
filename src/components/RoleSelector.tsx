import { Radio, Server } from "lucide-react";
import type { AppRole } from "../types";

export interface RoleSelectorProps {
  role: AppRole;
  onChangeRole: (role: AppRole) => void;
  disabled?: boolean;
}

export function RoleSelector({
  role,
  onChangeRole,
  disabled = false,
}: RoleSelectorProps) {
  return (
    <div className="grid grid-cols-2 p-1 bg-slate-900 rounded-xl border border-slate-800 shadow-sm">
      <button
        type="button"
        disabled={disabled}
        onClick={() => onChangeRole("client")}
        className={`flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
          role === "client"
            ? "bg-emerald-600 text-white shadow-lg shadow-emerald-950/60"
            : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 disabled:opacity-40 disabled:hover:bg-transparent cursor-pointer disabled:cursor-not-allowed"
        }`}
      >
        <Radio className="w-4 h-4" />
        <span>Client (Sender)</span>
      </button>

      <button
        type="button"
        disabled={disabled}
        onClick={() => onChangeRole("host")}
        className={`flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg text-sm font-medium transition-all ${
          role === "host"
            ? "bg-emerald-600 text-white shadow-lg shadow-emerald-950/60"
            : "text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 disabled:opacity-40 disabled:hover:bg-transparent cursor-pointer disabled:cursor-not-allowed"
        }`}
      >
        <Server className="w-4 h-4" />
        <span>Host (Receiver)</span>
      </button>
    </div>
  );
}
