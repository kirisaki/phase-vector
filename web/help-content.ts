// Help-only translations; game controls retain their English labels.
export type HelpLanguage = 'ja' | 'en';
export type HelpTopic = 'quick' | 'controls' | 'lua';
type Text = Record<HelpLanguage, string>;
interface Entry { title: Text; text: Text; code?: string }
interface Group { id: string; title: Text; entries: Entry[]; open?: boolean }
interface Section { title: Text; intro: Text; steps?: Entry[]; groups?: Group[] }
const t = (ja: string, en: string): Text => ({ ja, en });

export const helpLabels = {
  title: t('ヘルプ', 'Help'),
  close: t('ヘルプを閉じる', 'Close help'),
  topics: t('ヘルプの項目', 'Help topics'),
  paused: t('ヘルプを閉じても一時停止のままです。Auto または Commit で再開できます。',
    'Playback stays paused after closing help. Resume with Auto or Commit.'),
  insert: t('入力する', 'Insert into REPL'),
  loading: t('Lua の準備ができると入力できます。', 'Available when Lua is ready.'),
};

export const helpContent: Record<HelpTopic, Section> = {
  quick: {
    title: t('はじめに', 'Quick start'),
    intro: t('敵艦が制限時間内に赤い EXIT 領域へ入るのを阻止しましょう。命中で無力化するか、回避を強いて到着を遅らせれば勝利です。',
      'Stop the enemy from entering the red EXIT region before the deadline. Win with a disabling hit or force evasions that delay arrival past the deadline.'),
    steps: [
      { title: t('1 · Aim で未来を狙う', '1 · Aim ahead'),
        text: t('Aim をドラッグして弾着位置を選びます。細かい調整は Fine aim。Track lead は観測から計算した迎撃位置へ狙いを戻します。',
          'Drag Aim to choose an impact point. Use Fine aim for small adjustments. Track lead restores the intercept predicted from observations.') },
      { title: t('2 · Queue shot で予約', '2 · Queue shot'),
        text: t('Queue ボタンで射撃を予約します。Live では次に発射できる時刻、タイミングバーでは選んだ発射時刻を使います。緑の印は再装填後の発射枠です。',
          'Use Queue to reserve a shot. Live uses the next available firing slot; the timeline chooses a firing time. Green marks show slots after reload.') },
      { title: t('3 · Commit で様子を見る', '3 · Commit and observe'),
        text: t('Commit は次の指揮時点まで再生して停止します。敵の回避を見て、狙いと発射時刻を調整しましょう。',
          'Commit plays to the next command boundary and pauses. Watch enemy evasions, then adjust your aim and firing time.') },
    ],
    groups: [{ id: 'victory', title: t('勝利のためのヒント', 'Blockade tips'), entries: [
      { title: t('小惑星を避けて射線を開く', 'Maneuver to clear asteroid cover'),
        text: t('茶色の A1 は移動する小惑星です。弾は遮られ、艦艇が衝突すると行動不能になります。Blocked は遮蔽の予告です。Burn で射撃位置を変え、時間バーで小惑星の未来位置を確認しましょう。たとえば最初の 2 区間で減速し、Coast に戻してから敵の逃げ道を狙えます。',
          'Brown A1 is a moving asteroid. It stops rounds and disables ships on contact. Blocked warns of a cover intercept. Change your firing position with Burn and scrub time to inspect the asteroid. Try slowing for two intervals, then Coast and place shots across the enemy escape route.') },
      { title: t('予測は確定した未来ではありません', 'A prediction is not a guarantee'),
        text: t('敵の予測航路は、遅れて届く観測からの推定です。敵は攻撃を観測すると回避します。同じ場所への連射だけでなく、逃げ道を塞ぐ配置を試しましょう。',
          'The enemy forecast extrapolates delayed observations. The enemy may evade once it observes attacks. Try covering escape routes instead of repeatedly targeting one point.') },
      { title: t('撃沈しなくても勝てます', 'You can win without a hit'),
        text: t('Δv の枯渇や一時的な航路逸脱だけでは勝利になりません。期限まで EXIT への到達を阻止する必要があります。無攻撃なら敵は突破します。',
          'Fuel exhaustion or a temporary detour alone does not win. Keep the enemy out of EXIT until the deadline. Without attacks, it breaks through.') },
    ] }],
  },
  controls: {
    title: t('操作', 'Controls'),
    intro: t('計画は一時停止中でも再生中でも編集できます。実行中の指揮区間に確定した命令は変更できません。',
      'Edit plans while paused or running. Orders committed to the active command interval cannot be changed.'),
    groups: [
      { id: 'playback', title: t('時間の進め方', 'Playback'), open: true, entries: [
        { title: t('Auto / Pause', 'Auto / Pause'), text: t('Auto は指揮時点をまたいで連続再生し、Pause で停止します。速度ボタンは再生の速さを変えます。',
          'Auto plays continuously across command boundaries; Pause stops it. Speed buttons change playback speed.') },
        { title: t('Commit / Skip / +10 s', 'Commit / Skip / +10 s'), text: t('Commit は次の指揮時点まで再生して停止。Skip は同じ時点へ瞬時に進めて停止。+10 s は停止中に最大 10 秒進め、指揮時点では止まります。',
          'Commit animates to the next command boundary and pauses. Skip advances there immediately and pauses. While paused, +10 s advances up to ten seconds, stopping at a command boundary.') },
        { title: t('Retry', 'Retry'), text: t('任務を最初からやり直します。進行中は確認画面が開きます。Lua で変更した開始時設定は引き継ぎ、途中の設定変更は破棄します。',
          'Restart the mission. An unfinished run asks for confirmation. Retry retains custom starting settings and discards mid-play parameter edits.') },
      ] },
      { id: 'weapons', title: t('照準と射撃', 'Aiming and firing'), entries: [
        { title: t('Aim / Fine aim / Track lead', 'Aim / Fine aim / Track lead'), text: t('Aim はマウスでも指でもドラッグできます。Fine aim の近くに拡大図が現れ、弾道と敵の予測航路を表示します。輪郭の艦影は弾着時点の予測、塗りつぶしの艦影は現在位置の推定です。枠端の矢印は範囲外の弾着予測位置を示します。Aim は予測迎撃点付近で吸着し、Shift を押すと吸着を解除できます。',
          'Drag Aim with a mouse or finger. An inset follows Fine aim and shows the ballistic path and observed enemy course. An outlined ship predicts the enemy at impact; a filled ship estimates its current position. An edge arrow indicates an arrival prediction outside the inset. Aim snaps near the predicted intercept; hold Shift to bypass snapping.') },
        { title: t('タイミングバー / Live', 'Timeline / Live'), text: t('バーで選ぶのは弾着時刻ではなく発射時刻です。Aim はそこから弾が飛ぶ時間を考慮します。水色は次の指揮時点まで、橙色はそれ以降。Live で自動の発射時刻に戻ります。',
          'The timeline selects firing time, not impact time. Aim also accounts for flight time. Cyan covers the current command window; amber covers later times. Live restores automatic firing time.') },
        { title: t('Queue / Clear queue', 'Queue / Clear queue'), text: t('Queue は 1 発を予約し、再装填と予約上限があります。実現不能な指定はボタン下に理由を表示します。Clear queue や個別の取消で未確定の予約を消せます。飛翔中の弾は取り消せません。',
          'Queue reserves one round, subject to reload and queue capacity. Unavailable shots show a reason below the button. Clear queue or individual cancellation removes editable reservations; flying rounds cannot be cancelled.') },
        { title: t('キーボード・タッチ補助', 'Keyboard and touch shortcuts'), text: t('Aim にフォーカスして矢印キーで微調整、Shift でさらに細かく、Home で追尾に戻します。バーの操作中は Q で予約でき、タッチでは別の指で Queue を押せます。',
          'Focus Aim and use arrow keys to adjust, Shift for finer steps, and Home to restore lead tracking. While the timeline is focused, Q queues a shot; on touch, tap Queue with another finger.') },
      ] },
      { id: 'navigation', title: t('機動とマップ', 'Maneuvers and map'), entries: [
        { title: t('Burn', 'Burn'), text: t('自艦の Burn ハンドルをドラッグし、方向と燃焼量を選んで離すと計画に反映します。次の指揮区間から同じ加速度を繰り返し、Δv を消費します。中心は惰性飛行で、自動停止はしません。',
          'Drag and release the ship’s Burn handle to plan direction and burn amount. The acceleration repeats from the next command interval and consumes Δv. The center means coasting, not automatic braking.') },
        { title: t('ドラッグ / ピンチ / 表示モード', 'Drag / pinch / view modes'), text: t('空いている場所をドラッグすると画面移動。二本指でズームと移動ができます。Overview は両艦を自動で収め、Own ship と Contact は各艦を追います。手動移動後は Overview で自動表示に戻れます。',
          'Drag empty map space to pan. Pinch with two fingers to zoom and pan. Overview automatically frames both ships; Own ship and Contact follow one ship. Select Overview again after manual navigation to restore automatic framing.') },
        { title: t('航路と命令周期', 'Routes and command cycles'), text: t('水色の点線は自艦の予定航路、灰色は惰性飛行、橙色は観測に基づく敵の予測です。自艦周囲の波紋は指揮周期を知らせます。Burn も矢印キー・Shift・Home で操作できます。',
          'Cyan dots show your planned route, gray shows coasting, and amber predicts the observed enemy. The pulse around your ship marks a command boundary. Burn also supports arrows, Shift, and Home.') },
      ] },
    ],
  },
  lua: {
    title: t('Lua REPL', 'Lua REPL'),
    intro: t('「入力する」は入力欄の内容を置き換えるだけで、実行しません。Run Lua または Ctrl / ⌘ + Enter で実行します。マップの距離表示は km ですが、Lua は m・m/s・m/s² を使います。',
      'Insert into REPL replaces the input without executing it. Run with Run Lua or Ctrl / ⌘ + Enter. The map uses km; Lua uses m, m/s, and m/s².'),
    groups: [
      { id: 'inspection', title: t('状態と観測の確認', 'Inspect state and observations'), open: true, entries: [
        { title: t('現在の状態', 'Current state'), text: t('game.state() は診断用の真の状態を含みます。返された表を編集してもゲームは変わりません。',
          'game.state() includes true state for diagnostics. Editing returned tables does not change the game.'), code: 'return game.state()' },
        { title: t('受信済みの敵観測', 'Received enemy observation'), text: t('通常画面の敵表示は、この遅れて届いた観測に基づきます。',
          'The normal enemy display uses this delayed observation.'), code: 'return game.state().mission.contact' },
        { title: t('敵 AI の判断理由', 'Enemy AI reason'), text: t('現在の判断理由と回避回数を診断します。',
          'Inspect the current decision reason and evasion count.'), code: 'local m = game.state().mission; return m.ai_reason, m.evasions' },
      ] },
      { id: 'commands', title: t('通常の命令', 'Normal commands'), entries: [
        { title: t('一時停止', 'Pause'), text: t('通常 UI と同じ命令 API を使います。', 'Uses the same command API as the UI.'), code: 'game.command("pause")' },
        { title: t('1 物理刻みだけ進める', 'Advance one physics tick'), text: t('一時停止中のみ実行できます。画面の +10 s より短い診断用ステップです。',
          'Requires paused playback. This diagnostic step is shorter than the UI’s +10 s.'), code: 'game.command("step"); return game.state().time_seconds' },
        { title: t('予測迎撃点へ射撃を予約', 'Queue an observed-lead shot'), text: t('Queue と同じ再装填・予約上限の検証を受けます。',
          'Uses the same reload and capacity validation as Queue.'), code: 'game.fire(game.aim())' },
        { title: t('惰性飛行を計画', 'Plan coasting'), text: t('次の指揮区間の燃焼をゼロにします。速度はゼロになりません。',
          'Set the next interval’s burn to zero. This does not set velocity to zero.'), code: 'game.burn(0, 0)' },
        { title: t('スコアと任務の区別', 'Score and run kind'), text: t('スコアは任務終了後に確定します。', 'The score is final only after the mission ends.'),
          code: 'local m = game.state().mission; return m.run_kind, m.evaluation' },
      ] },
      { id: 'settings', title: t('バランス設定と Custom / Debug', 'Balance settings and Custom / Debug'), entries: [
        { title: t('設定値を読む', 'Read settings'), text: t('設定の確認だけでは任務の区別は変わりません。',
          'Reading settings does not change the run kind.'), code: 'return game.parameters()' },
        { title: t('停止中の設定変更', 'Edit settings while paused'), text: t('変更は未終了の任務で一時停止中のみ可能です。開始前の変更は Custom settings、開始後の変更は Debug run。途中で値を戻しても Debug は残り、スコアは診断用の参考値になります。',
          'Edits require a paused, unfinished mission. Before play they produce Custom settings; during play they mark a Debug run. Reverting a mid-play edit keeps Debug, and the score is diagnostic.'),
          code: 'game.configure("reaction_seconds", 60)' },
        { title: t('開始前のみ・予約が空のとき', 'Before play, with an empty queue'), text: t('deadline_seconds：期限 0.25–7,200 秒。reload_seconds：再装填 0.25–600 秒。sensor_processing_seconds：観測処理遅延 0–120 秒。いずれも 0.25 秒の倍数です。',
          'deadline_seconds: deadline, 0.25–7,200 s. reload_seconds: reload, 0.25–600 s. sensor_processing_seconds: observation processing delay, 0–120 s. Each must be a multiple of 0.25 s.') },
        { title: t('開始後も変更できる AI 設定', 'AI settings editable during play'), text: t('reaction_seconds：脅威の予測時間 0–600 秒。safety_radius_m：安全距離 0–2,000,000 m。hold_seconds：回避継続時間 0–120 秒。recovery_seconds：航路復帰の調整時間 0.25–1,200 秒。',
          'reaction_seconds: threat horizon, 0–600 s. safety_radius_m: safety margin, 0–2,000,000 m. hold_seconds: evasion hold, 0–120 s. recovery_seconds: cruise recovery timescale, 0.25–1,200 s.') },
        { title: t('標準設定からやり直す', 'Restart with standard settings'), text: t('任務をリセットし、標準設定に戻します。Retry は開始時の設定を引き継ぐので、Custom から標準に戻す場合はこちらを使います。',
          'Resets the mission to standard settings. Retry retains starting settings; use this to leave a custom baseline.'), code: 'game.command("mission")' },
      ] },
    ],
  },
};
