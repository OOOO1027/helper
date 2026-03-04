export function nowLocalIso(): string {
  return new Date().toISOString();
}

export function sevenDaysAgoIso(): string {
  const d = new Date();
  d.setDate(d.getDate() - 7);
  return d.toISOString();
}

export function formatPercent(input: number): string {
  return `${Math.round(input * 100)}%`;
}

export function formatTime(input: string): string {
  const d = new Date(input);
  if (Number.isNaN(d.getTime())) {
    return input;
  }
  return d.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  });
}
