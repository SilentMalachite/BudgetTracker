export function isoToday(): string {
  return new Date().toISOString().slice(0, 10);
}

export function monthRange(year: number, month: number): { from: string; to: string } {
  const m = String(month).padStart(2, '0');
  const last = new Date(year, month, 0).getDate();
  return {
    from: `${year}-${m}-01`,
    to: `${year}-${m}-${String(last).padStart(2, '0')}`,
  };
}
