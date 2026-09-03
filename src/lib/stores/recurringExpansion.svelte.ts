import { expandDueRecurring, type ExpansionResult } from '../api/recurring';

/**
 * 直近の定期取引展開の結果を 1 か所で持つ。
 *
 * 起動時展開 (App) と保存後・再試行の展開 (Recurring 画面) が別々に結果を抱えると、
 * 参照先を直したあとにアプリシェルのバナーだけが古い件数を出し続ける。バナー・
 * 行バッジ・再試行が同じ値を読むように、他のストアと違ってこれはモジュール単位の
 * シングルトンにしてある。
 */
export type RecurringExpansionStore = {
  /** 最後に成功した展開の結果。まだ 1 度も成功していなければ null。 */
  readonly result: ExpansionResult | null;
  /** 最後の展開が失敗していればそのメッセージ。成功するまで消えない。 */
  readonly error: string | null;
  readonly running: boolean;
  /** 展開を 1 回走らせる。失敗しても throw せず false を返す (起動を止めないため)。 */
  run(): Promise<boolean>;
};

let result = $state<ExpansionResult | null>(null);
let error = $state<string | null>(null);
let running = $state(false);

async function run(): Promise<boolean> {
  running = true;
  try {
    result = await expandDueRecurring();
    error = null;
    return true;
  } catch (e) {
    // 展開の失敗でアプリを開けなくしない。代わりにメッセージを残し、
    // Recurring 画面の再試行ボタンから同じ run() を呼び直せるようにする。
    error = e instanceof Error ? e.message : String(e);
    return false;
  } finally {
    running = false;
  }
}

export const recurringExpansion: RecurringExpansionStore = {
  get result() {
    return result;
  },
  get error() {
    return error;
  },
  get running() {
    return running;
  },
  run,
};
