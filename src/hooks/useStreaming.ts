import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  StreamStatus,
  AudioLevelPayload,
  StreamTelemetry,
  TransportMode,
} from "../types";

export interface UseStreamingResult {
  status: StreamStatus;
  audioLevel: AudioLevelPayload;
  telemetry: StreamTelemetry;
  isStarting: boolean;
  isStopping: boolean;
  error: string | null;
  startStreaming: (
    targetIp: string,
    targetPort: number,
    mode?: TransportMode,
    deviceName?: string
  ) => Promise<void>;
  stopStreaming: () => Promise<void>;
  startListening: (
    port: number,
    outputDeviceName?: string
  ) => Promise<void>;
  stopListening: () => Promise<void>;
  clearError: () => void;
}

const DEFAULT_AUDIO_LEVEL: AudioLevelPayload = {
  input_level: 0,
  output_level: 0,
  peak: 0,
};

const DEFAULT_TELEMETRY: StreamTelemetry = {
  rtt_ms: 0,
  packet_loss_percent: 0,
  packets_sent: 0,
  packets_lost: 0,
  jitter_ms: 0,
};

export function useStreaming(): UseStreamingResult {
  const [status, setStatus] = useState<StreamStatus>("Idle");
  const [audioLevel, setAudioLevel] = useState<AudioLevelPayload>(DEFAULT_AUDIO_LEVEL);
  const [telemetry, setTelemetry] = useState<StreamTelemetry>(DEFAULT_TELEMETRY);
  const [isStarting, setIsStarting] = useState<boolean>(false);
  const [isStopping, setIsStopping] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const isMountedRef = useRef<boolean>(true);

  const clearError = useCallback(() => {
    setError(null);
  }, []);

  // Subscribe to backend events on mount and fetch initial status
  useEffect(() => {
    isMountedRef.current = true;
    let isCleanedUp = false;

    let unlistenAudioLevel: UnlistenFn | undefined;
    let unlistenStreamStatus: UnlistenFn | undefined;
    let unlistenTelemetry: UnlistenFn | undefined;

    // Fetch initial status
    invoke<StreamStatus>("get_stream_status")
      .then((initialStatus) => {
        if (!isCleanedUp) {
          setStatus(initialStatus);
        }
      })
      .catch((err) => {
        console.warn("Failed to query initial stream status:", err);
      });

    // Listen for 60 Hz audio-level events
    listen<AudioLevelPayload>("audio-level", (event) => {
      if (!isCleanedUp) {
        setAudioLevel(event.payload);
      }
    }).then((unlisten) => {
      if (isCleanedUp) {
        unlisten();
      } else {
        unlistenAudioLevel = unlisten;
      }
    });

    // Listen for stream-status changes
    listen<StreamStatus>("stream-status", (event) => {
      if (!isCleanedUp) {
        setStatus(event.payload);
        if (event.payload === "Idle" || event.payload === "Error") {
          setAudioLevel(DEFAULT_AUDIO_LEVEL);
        }
      }
    }).then((unlisten) => {
      if (isCleanedUp) {
        unlisten();
      } else {
        unlistenStreamStatus = unlisten;
      }
    });

    // Listen for 1 Hz telemetry-update events
    listen<StreamTelemetry>("telemetry-update", (event) => {
      if (!isCleanedUp) {
        setTelemetry(event.payload);
      }
    }).then((unlisten) => {
      if (isCleanedUp) {
        unlisten();
      } else {
        unlistenTelemetry = unlisten;
      }
    });

    return () => {
      isCleanedUp = true;
      isMountedRef.current = false;
      unlistenAudioLevel?.();
      unlistenStreamStatus?.();
      unlistenTelemetry?.();
    };
  }, []);

  const startStreaming = useCallback(
    async (
      targetIp: string,
      targetPort: number,
      mode: TransportMode = "opus",
      deviceName?: string
    ) => {
      setIsStarting(true);
      setError(null);

      try {
        await invoke("start_stream", {
          targetIp,
          targetPort,
          mode,
          deviceName: deviceName ?? null,
        });
      } catch (err) {
        if (isMountedRef.current) {
          const message = err instanceof Error ? err.message : String(err);
          setError(message);
        }
        throw err;
      } finally {
        if (isMountedRef.current) {
          setIsStarting(false);
        }
      }
    },
    []
  );

  const stopStreaming = useCallback(async () => {
    setIsStopping(true);
    setError(null);

    try {
      await invoke("stop_stream");
      if (isMountedRef.current) {
        setAudioLevel(DEFAULT_AUDIO_LEVEL);
      }
    } catch (err) {
      if (isMountedRef.current) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
      }
      throw err;
    } finally {
      if (isMountedRef.current) {
        setIsStopping(false);
      }
    }
  }, []);

  const startListening = useCallback(
    async (port: number, outputDeviceName?: string) => {
      setIsStarting(true);
      setError(null);

      try {
        await invoke("start_listen", {
          port,
          outputDeviceName: outputDeviceName ?? null,
        });
      } catch (err) {
        if (isMountedRef.current) {
          const message = err instanceof Error ? err.message : String(err);
          setError(message);
        }
        throw err;
      } finally {
        if (isMountedRef.current) {
          setIsStarting(false);
        }
      }
    },
    []
  );

  const stopListening = useCallback(async () => {
    setIsStopping(true);
    setError(null);

    try {
      await invoke("stop_listen");
      if (isMountedRef.current) {
        setAudioLevel(DEFAULT_AUDIO_LEVEL);
      }
    } catch (err) {
      if (isMountedRef.current) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
      }
      throw err;
    } finally {
      if (isMountedRef.current) {
        setIsStopping(false);
      }
    }
  }, []);

  return {
    status,
    audioLevel,
    telemetry,
    isStarting,
    isStopping,
    error,
    startStreaming,
    stopStreaming,
    startListening,
    stopListening,
    clearError,
  };
}
