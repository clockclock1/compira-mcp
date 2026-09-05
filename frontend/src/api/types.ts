export interface User {
  id: string;
  username: string;
  role: string;
  display_name: string | null;
  created_at: string;
  last_login_at: string | null;
}

export interface PropDef {
  name: string;
  type?: string;
  default?: string;
  required: boolean;
  description?: string;
}

export interface EventDef {
  name: string;
  payload?: string;
  description?: string;
}

export interface SlotDef {
  name: string;
  description?: string;
}

export interface Component {
  id: string;
  library_id: string;
  name: string;
  file_path: string;
  framework: string;
  description?: string;
  props: PropDef[];
  events: EventDef[];
  slots: SlotDef[];
  tags: string[];
}

export interface ComponentExample {
  id: string;
  component_id: string;
  title: string;
  code: string;
}

export interface ComponentSearchResult {
  id: string;
  library_id: string;
  library_name: string;
  name: string;
  file_path: string;
  framework: string;
  description?: string;
  score: number;
}

export interface McpTool {
  name: string;
  description: string;
  parameters: Record<string, string>;
}
