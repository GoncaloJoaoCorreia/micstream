export interface AudioDeviceInfo {
  id: string;
  name: string;
  is_default: boolean;
  is_virtual: boolean;
}

export interface VirtualDriverStatus {
  detected: boolean;
  driver_name: string;
  install_url: string;
  instructions: string;
}

export type StreamStatus =
  | "Idle"
  | "Streaming"
  | "Listening"
  | "Disconnected"
  | "Error";

export interface AudioLevelPayload {
  input_level: number;
  output_level: number;
  peak: number;
}

export interface StreamTelemetry {
  rtt_ms: number;
  packet_loss_percent: number;
  packets_sent: number;
  packets_lost: number;
  jitter_ms: number;
}

export interface DiscoveredHost {
  host_name: string;
  ip_addresses: string[];
  port: number;
}

export type AppRole = "client" | "host";
export type TransportMode = "opus" | "raw_pcm";

export interface RecentHost {
  host_name?: string;
  ip: string;
  port: number;
  last_connected?: number;
}

export interface AppSettings {
  mode: TransportMode;
  target_jitter_ms: number;
  udp_port: number;
}
