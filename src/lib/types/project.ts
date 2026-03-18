export type ProjectType = 'single_file' | 'aidps_folder' | 'vibproj_file';

export interface DataSource {
  file_path: string;
  file_name: string;
  source_type: 'csv' | 'wav';
}

export interface ChannelSchema {
  groups: Record<string, string[]>;
}

export interface DeviceInfo {
  id: string;
  name: string;
  sources: DataSource[];
  channel_schema: ChannelSchema;
}

export interface ProjectMetadata {
  name: string;
  created_at: string;
  description?: string;
}

export interface ProjectInfo {
  project_type: ProjectType;
  devices: DeviceInfo[];
  sensor_mapping: Record<string, string>;
  metadata: ProjectMetadata;
}

export interface SpectrumData {
  frequencies: number[];
  amplitudes: number[];
  sample_rate: number;
  fft_size: number;
}
