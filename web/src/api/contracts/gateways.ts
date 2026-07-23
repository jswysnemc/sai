export type GatewayStatus = {
  id: string;
  title: string;
  enabled: boolean;
  task_id?: string | null;
  status: string;
  pid?: number | null;
};

export type WeixinLoginPhase =
  | "waiting"
  | "scanned"
  | "need_verify_code"
  | "confirmed"
  | "expired"
  | "failed";

export type WeixinLoginAccount = {
  account_id: string;
  base_url: string;
  cdn_base_url: string;
  user_id?: string | null;
};

export type WeixinLoginSnapshot = {
  session_id: string;
  phase: WeixinLoginPhase;
  qrcode_content: string;
  qrcode_svg: string;
  message?: string | null;
  account?: WeixinLoginAccount | null;
};
