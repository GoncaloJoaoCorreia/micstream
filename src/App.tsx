import { useState, useEffect, useCallback } from "react";
import {
  AlertCircle,
  Play,
  Square,
  Activity,
  Wifi,
  Radio,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { AppRole, AppSettings } from "./types";
import { useAudioDevices } from "./hooks/useAudioDevices";
import { useStreaming } from "./hooks/useStreaming";
import { useDiscovery } from "./hooks/useDiscovery";
import {
  Header,
  RoleSelector,
  DevicePicker,
  VolumeMeter,
  DriverSetupCard,
  HostDiscoveryList,
  ConnectionStats,
  SettingsModal,
  DEFAULT_APP_SETTINGS,
} from "./components";

const STORAGE_KEY_SETTINGS = "micstream:config";

function loadSettingsFromStorage(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY_SETTINGS);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (
        parsed &&
        (parsed.mode === "opus" || parsed.mode === "raw_pcm") &&
        typeof parsed.target_jitter_ms === "number" &&
        typeof parsed.udp_port === "number"
      ) {
        return {
          mode: parsed.mode,
          target_jitter_ms: parsed.target_jitter_ms,
          udp_port: parsed.udp_port,
        };
      }
    }
  } catch (err) {
    console.warn("Failed to load settings from storage:", err);
  }
  return DEFAULT_APP_SETTINGS;
}

function saveSettingsToStorage(settings: AppSettings) {
  try {
    localStorage.setItem(STORAGE_KEY_SETTINGS, JSON.stringify(settings));
  } catch (err) {
    console.warn("Failed to save settings to storage:", err);
  }
}

export function App() {
  const [role, setRole] = useState<AppRole>("client");
  const [settings, setSettings] = useState<AppSettings>(loadSettingsFromStorage);
  const [manualIp, setManualIp] = useState<string>("127.0.0.1");
  const [manualPort, setManualPort] = useState<number>(settings.udp_port);
  const [isSettingsOpen, setIsSettingsOpen] = useState<boolean>(false);

  // Sync target jitter watermark to backend on mount and settings change
  useEffect(() => {
    invoke("set_target_jitter", {
      jitterMs: settings.target_jitter_ms,
    }).catch((err) => {
      console.warn("Failed to update backend jitter watermark:", err);
    });
  }, [settings.target_jitter_ms]);

  const {
    inputDevices,
    outputDevices,
    selectedInput,
    selectedOutput,
    virtualDriverStatus,
    isLoading: isDevicesLoading,
    error: devicesError,
    refreshDevices,
    setSelectedInput,
    setSelectedOutput,
  } = useAudioDevices();

  const {
    status,
    audioLevel,
    telemetry,
    isStarting,
    isStopping,
    error: streamError,
    startStreaming,
    stopStreaming,
    startListening,
    stopListening,
    clearError: clearStreamError,
  } = useStreaming();

  const {
    isDiscovering,
    discoveredHosts,
    recentHosts,
    error: discoveryError,
    startDiscovery,
    stopDiscovery,
    addRecentHost,
    removeRecentHost,
    clearDiscoveredHosts,
  } = useDiscovery();

  const isStreaming = status === "Streaming";
  const isListening = status === "Listening";
  const isActive = isStreaming || isListening;

  // Manage discovery based on role and active state
  useEffect(() => {
    if (role === "client" && !isActive) {
      startDiscovery();
    } else {
      stopDiscovery();
    }
  }, [role, isActive, startDiscovery, stopDiscovery]);

  const handleRoleChange = useCallback(
    (newRole: AppRole) => {
      if (isActive) return;
      setRole(newRole);
      if (newRole === "host") {
        stopDiscovery();
      } else {
        clearDiscoveredHosts();
        startDiscovery();
      }
    },
    [isActive, stopDiscovery, clearDiscoveredHosts, startDiscovery]
  );

  const handleSaveSettings = useCallback((newSettings: AppSettings) => {
    setSettings(newSettings);
    saveSettingsToStorage(newSettings);
    setManualPort(newSettings.udp_port);
  }, []);

  const handleConnectClient = useCallback(
    async (ip: string, port: number, hostName?: string) => {
      clearStreamError();
      try {
        await startStreaming(ip, port, settings.mode, selectedInput);
        addRecentHost(ip, port, hostName);
      } catch (err) {
        console.error("Failed to connect client:", err);
      }
    },
    [clearStreamError, startStreaming, settings.mode, selectedInput, addRecentHost]
  );

  const handleToggleStream = async () => {
    clearStreamError();
    if (role === "client") {
      if (isStreaming) {
        await stopStreaming();
      } else {
        await handleConnectClient(manualIp, manualPort);
      }
    } else {
      if (isListening) {
        await stopListening();
      } else {
        await startListening(settings.udp_port, selectedOutput);
      }
    }
  };

  const handleDismissErrors = () => {
    clearStreamError();
  };

  const currentLevel =
    role === "client" ? audioLevel.input_level : audioLevel.output_level;

  const combinedError = streamError || devicesError || discoveryError;

  return (
    <div className="flex flex-col h-screen bg-slate-950 text-slate-100 select-none overflow-y-auto">
      {/* 1. Header with Status & Settings Trigger */}
      <Header
        status={status}
        isActive={isActive}
        isStreaming={isStreaming}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      {/* 2. Main Dashboard Content */}
      <main className="p-6 max-w-xl mx-auto w-full flex flex-col gap-5 flex-1">
        {/* Error Notification Banner */}
        {combinedError && (
          <div className="flex items-start justify-between gap-3 p-3 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-300 text-xs">
            <div className="flex items-center gap-2">
              <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
              <span>{combinedError}</span>
            </div>
            <button
              type="button"
              onClick={handleDismissErrors}
              className="text-rose-400 hover:text-rose-200 font-medium cursor-pointer"
            >
              Dismiss
            </button>
          </div>
        )}

        {/* Role Switcher (Client vs Host) */}
        <RoleSelector
          role={role}
          onChangeRole={handleRoleChange}
          disabled={isActive}
        />

        {/* Host Mode Virtual Driver Diagnostic Card */}
        {role === "host" && (
          <DriverSetupCard
            status={virtualDriverStatus}
            onRefresh={refreshDevices}
            isRefreshing={isDevicesLoading}
          />
        )}

        {/* Audio Device Selector */}
        {role === "client" ? (
          <DevicePicker
            label="Microphone Input (Shared)"
            type="input"
            devices={inputDevices}
            selectedDevice={selectedInput}
            onSelectDevice={setSelectedInput}
            onRefresh={refreshDevices}
            isLoading={isDevicesLoading}
            disabled={isActive}
          />
        ) : (
          <DevicePicker
            label="Audio Output Sink (Virtual Cable)"
            type="output"
            devices={outputDevices}
            selectedDevice={selectedOutput}
            onSelectDevice={setSelectedOutput}
            onRefresh={refreshDevices}
            isLoading={isDevicesLoading}
            disabled={isActive}
          />
        )}

        {/* Client Mode: LAN Discovery & Manual IP Input */}
        {role === "client" && (
          <HostDiscoveryList
            discoveredHosts={discoveredHosts}
            recentHosts={recentHosts}
            isDiscovering={isDiscovering}
            manualIp={manualIp}
            manualPort={manualPort}
            onManualIpChange={setManualIp}
            onManualPortChange={setManualPort}
            onConnect={handleConnectClient}
            onRemoveRecentHost={removeRecentHost}
            onRefreshDiscovery={() => {
              clearDiscoveredHosts();
              startDiscovery();
            }}
            disabled={isActive}
            isConnecting={isStarting}
          />
        )}

        {/* Host Mode: Network Listener Information */}
        {role === "host" && (
          <div className="p-4 rounded-xl bg-slate-900/70 border border-slate-800 flex flex-col gap-2.5 shadow-sm">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className="p-1 rounded bg-emerald-500/10 text-emerald-400">
                  <Wifi className="w-4 h-4" />
                </div>
                <span className="text-xs font-semibold text-slate-200">
                  Host Service Status
                </span>
              </div>
              <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700">
                UDP Port: {settings.udp_port}
              </span>
            </div>

            <p className="text-xs text-slate-400">
              When listening is started, this computer will broadcast its presence over mDNS (
              <span className="font-mono text-slate-300">_micstream._udp.local.</span>) for one-click discovery by clients on the LAN.
            </p>

            <div className="flex items-center gap-2 pt-1 text-[11px] text-slate-500">
              <Radio className="w-3.5 h-3.5 text-emerald-500" />
              <span>
                Jitter buffer target: <span className="font-mono text-slate-300">{settings.target_jitter_ms.toFixed(1)}ms</span> (configured in settings).
              </span>
            </div>
          </div>
        )}

        {/* Live 60 Hz VU Volume Meter */}
        <VolumeMeter
          level={currentLevel}
          peak={audioLevel.peak}
          label={role === "client" ? "Microphone Capture Level" : "Output Playback Level"}
          isActive={isActive}
        />

        {/* Real-time Connection Telemetry Badge */}
        <ConnectionStats
          telemetry={telemetry}
          mode={settings.mode}
          isActive={isActive}
          role={role}
        />

        {/* Primary Start / Stop Action Button */}
        <div className="pt-1 pb-4">
          <button
            type="button"
            onClick={handleToggleStream}
            disabled={isStarting || isStopping}
            className={`w-full py-3.5 px-4 rounded-xl font-medium text-sm transition flex items-center justify-center gap-2 shadow-lg disabled:opacity-50 cursor-pointer ${
              isActive
                ? "bg-rose-600 hover:bg-rose-500 text-white shadow-rose-950/60"
                : "bg-emerald-600 hover:bg-emerald-500 text-white shadow-emerald-950/60"
            }`}
          >
            {isStarting || isStopping ? (
              <>
                <Activity className="w-4 h-4 animate-spin" />
                <span>{isStarting ? "Connecting..." : "Stopping..."}</span>
              </>
            ) : isActive ? (
              <>
                <Square className="w-4 h-4 fill-current" />
                <span>{role === "client" ? "Stop Streaming" : "Stop Listening"}</span>
              </>
            ) : role === "client" ? (
              <>
                <Play className="w-4 h-4 fill-current" />
                <span>Stream to {manualIp}:{manualPort}</span>
              </>
            ) : (
              <>
                <Play className="w-4 h-4 fill-current" />
                <span>Start Listening on Port {settings.udp_port}</span>
              </>
            )}
          </button>
        </div>
      </main>

      {/* 3. Settings Modal */}
      <SettingsModal
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        settings={settings}
        onSave={handleSaveSettings}
        disabled={isActive}
      />
    </div>
  );
}

export default App;
