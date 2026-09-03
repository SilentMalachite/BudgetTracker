import { describe, expect, it } from 'vitest';

import { readyBootCommands, readyBootResult } from './e2e/tauriMock';
import bootStatus from './fixtures/responses/boot_status.ready.json';
import expandDueRecurring from './fixtures/responses/expand_due_recurring.json';
import listBalances from './fixtures/responses/list_balances.json';
import listTransactions from './fixtures/responses/list_transactions.json';
import monthlySummary from './fixtures/responses/monthly_summary.json';
import reportMonthly from './fixtures/responses/report_monthly.json';
import reportYearly from './fixtures/responses/report_yearly.json';

/**
 * The ready-boot mock answers every command with its empty state. Object
 * responses must carry exactly the key set Rust serializes (fixtures generated
 * by `src-tauri/tests/response_fixtures.rs`); list responses are `[]`.
 */
const objectResponses: Record<string, Record<string, unknown>> = {
  boot_status: bootStatus,
  list_balances: listBalances,
  monthly_summary: monthlySummary,
  list_transactions: listTransactions,
  expand_due_recurring: expandDueRecurring,
  report_monthly: reportMonthly,
  report_yearly: reportYearly,
};

const listResponses = [
  'list_budget_statuses',
  'list_top_budget_statuses',
  'monthly_series',
  'list_categories',
  'list_accounts',
  'list_recurring_rules',
];

describe('ready boot e2e mock', () => {
  it('pins every mocked command to a fixture', () => {
    expect([...readyBootCommands()].sort()).toEqual(
      [...Object.keys(objectResponses), ...listResponses].sort(),
    );
  });

  it.each(Object.keys(objectResponses))('%s has the fixture key set', (command) => {
    const fixture = objectResponses[command];
    const result = readyBootResult(command) as Record<string, unknown>;

    expect(Object.keys(result).sort()).toEqual(Object.keys(fixture).sort());
    for (const [key, value] of Object.entries(fixture)) {
      // Nested lists are empty in the ready state; scalars keep the fixture's primitive type.
      if (Array.isArray(value)) expect(result[key]).toEqual([]);
      else if (value !== null && result[key] !== null) expect(typeof result[key]).toBe(typeof value);
    }
  });

  it.each(listResponses)('%s is an empty list', (command) => {
    expect(readyBootResult(command)).toEqual([]);
  });

  it('boots straight into the ready state', () => {
    expect(readyBootResult('boot_status')).toMatchObject({ state: 'ready', recovery_reason: null });
  });

  it('answers null for commands it does not mock', () => {
    expect(readyBootResult('not_a_command')).toBeNull();
  });
});
