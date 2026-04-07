import { chromium } from '/opt/node22/lib/node_modules/playwright/index.mjs';
import { mkdir } from 'fs/promises';
import { existsSync } from 'fs';

const SCREENSHOTS_DIR = './docs/screenshots';
const BASE_URL = 'http://localhost:5173';

async function takeScreenshots() {
  if (!existsSync(SCREENSHOTS_DIR)) {
    await mkdir(SCREENSHOTS_DIR, { recursive: true });
  }

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();

  page.on('console', msg => {
    if (msg.type() === 'error') console.log('PAGE ERROR:', msg.text().substring(0, 150));
  });
  page.on('pageerror', err => console.log('PAGE EXCEPTION:', err.message.substring(0, 150)));

  // Mock all Tauri APIs with correct return values
  await page.addInitScript(() => {
    let ridCounter = 1;

    const mockInvoke = async (cmd, args) => {
      // ---- Store plugin ----
      if (cmd === 'plugin:store|load') return ridCounter++;
      if (cmd === 'plugin:store|get') return [null, false];  // [value, exists]
      if (cmd === 'plugin:store|set') return null;
      if (cmd === 'plugin:store|save') return null;
      if (cmd === 'plugin:store|has') return false;
      if (cmd === 'plugin:store|delete') return false;
      if (cmd === 'plugin:store|keys') return [];
      if (cmd === 'plugin:store|values') return [];
      if (cmd === 'plugin:store|entries') return [];
      if (cmd === 'plugin:store|length') return 0;
      if (cmd === 'plugin:store|clear') return null;
      if (cmd === 'plugin:store|reset') return null;
      if (cmd === 'plugin:store|reload') return null;
      if (cmd === 'plugin:store|close') return null;
      if (cmd === 'plugin:store|get_store') return null;

      // ---- SQL plugin ----
      if (cmd === 'plugin:sql|load') return args?.db ?? 'sqlite:mock.db';
      if (cmd === 'plugin:sql|execute') return [0, 0];  // [rowsAffected, lastInsertId]
      if (cmd === 'plugin:sql|select') return [];
      if (cmd === 'plugin:sql|close') return null;

      // ---- Dialog plugin ----
      if (cmd === 'plugin:dialog|open') return null;
      if (cmd === 'plugin:dialog|save') return null;

      // ---- Shell/opener plugin ----
      if (cmd === 'plugin:shell|open') return null;
      if (cmd === 'plugin:opener|open_url') return null;

      // ---- Event system ----
      if (cmd === 'plugin:event|listen') return ridCounter++;
      if (cmd === 'plugin:event|unlisten') return null;
      if (cmd === 'plugin:event|emit') return null;
      if (cmd === 'plugin:event|emit_to') return null;

      // ---- Custom app commands ----
      if (cmd === 'get_projects') return [];
      if (cmd === 'get_stories') return [];
      if (cmd === 'get_archived_stories') return [];
      if (cmd === 'get_tasks') return [];
      if (cmd === 'get_archived_tasks') return [];
      if (cmd === 'get_tasks_by_story_id') return [];
      if (cmd === 'get_sprints') return [];
      if (cmd === 'get_all_task_dependencies') return [];
      if (cmd === 'get_active_claude_sessions') return [];
      if (cmd === 'get_team_chat_messages') return [];
      if (cmd === 'get_available_models') return ['claude-opus-4-6', 'claude-sonnet-4-6'];
      if (cmd === 'get_team_configuration') return { max_concurrent_agents: 1, roles: [] };
      if (cmd === 'check_scaffold_status') return { exists: false };
      if (cmd === 'detect_tech_stack') return { detected: false, stacks: [] };
      if (cmd === 'read_inception_file') return null;
      if (cmd === 'create_project') return null;
      if (cmd === 'delete_project') return null;
      if (cmd === 'update_project_path') return { success: true, has_product_context: false, has_architecture: false, has_rule: false };
      if (cmd === 'add_story') return null;
      if (cmd === 'update_story') return null;
      if (cmd === 'delete_story') return null;
      if (cmd === 'add_task') return null;
      if (cmd === 'update_task') return null;
      if (cmd === 'update_task_status') return null;
      if (cmd === 'delete_task') return null;
      if (cmd === 'assign_story_to_sprint') return null;
      if (cmd === 'assign_task_to_sprint') return null;
      if (cmd === 'set_task_dependencies') return null;
      if (cmd === 'create_planned_sprint') return { id: 'mock-sprint', name: 'Sprint 1', status: 'planned' };
      if (cmd === 'start_sprint') return null;
      if (cmd === 'complete_sprint') return null;
      if (cmd === 'archive_sprint') return null;
      if (cmd === 'add_team_chat_message') return null;
      if (cmd === 'clear_team_chat_messages') return null;
      if (cmd === 'save_team_configuration') return null;
      if (cmd === 'execute_claude_task') return null;
      if (cmd === 'kill_claude_process') return null;
      if (cmd === 'generate_base_rule') return null;
      if (cmd === 'generate_agent_md') return '';
      if (cmd === 'generate_claude_settings') return null;
      if (cmd === 'execute_scaffold_cli') return false;
      if (cmd === 'execute_scaffold_ai') return null;
      if (cmd === 'write_inception_file') return null;
      if (cmd === 'chat_with_team_leader') return { reply: '' };
      if (cmd === 'pty_spawn') return 'mock-pty';
      if (cmd === 'pty_execute') return { output: '' };
      if (cmd === 'pty_kill') return null;

      console.log('[MOCK] Unhandled invoke:', cmd);
      return null;
    };

    window.__TAURI_INTERNALS__ = {
      invoke: mockInvoke,
      transformCallback: (cb, once) => {
        return Math.ceil(Math.random() * 1000000);
      },
      convertFileSrc: (src) => src,
      metadata: {
        currentWindow: { label: 'main' },
        windows: [{ label: 'main' }],
      },
    };

    window.__TAURI__ = {
      core: { invoke: mockInvoke },
      event: {
        listen: async () => (() => {}),
        once: async () => (() => {}),
        emit: async () => {},
      },
      window: {
        getCurrent: () => ({
          label: 'main',
          listen: async () => (() => {}),
        }),
      },
    };
  });

  console.log('Navigating to', BASE_URL);
  await page.goto(BASE_URL, { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(4000);

  const buttonCount = await page.locator('button').count();
  console.log('Found buttons:', buttonCount);

  if (buttonCount === 0) {
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/debug_blank.png` });
    const html = await page.content();
    console.log('Page HTML (first 500 chars):', html.substring(0, 500));
    await browser.close();
    return;
  }

  // 1. Kanban view (default)
  console.log('Taking screenshot: 01_kanban_view');
  await page.screenshot({ path: `${SCREENSHOTS_DIR}/01_kanban_view.png` });

  // 2. AI Leader Sidebar
  console.log('Taking screenshot: 02_ai_leader_sidebar');
  const aiLeaderBtn = page.locator('button').filter({ hasText: /AI Leader/i }).first();
  if (await aiLeaderBtn.count() > 0) {
    await aiLeaderBtn.click();
    await page.waitForTimeout(800);
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/02_ai_leader_sidebar.png` });
    await aiLeaderBtn.click();
    await page.waitForTimeout(500);
  }

  // 3. Terminal open - find the terminal toggle bar
  console.log('Taking screenshot: 03_terminal_open');
  // The terminal toggle is likely a button or clickable div at the bottom
  const termBtn = page.locator('button').filter({ hasText: /TERMINAL|Terminal/i }).first();
  const termCount = await termBtn.count();
  if (termCount > 0) {
    await termBtn.click();
    await page.waitForTimeout(800);
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/03_terminal_open.png` });
  } else {
    // Try finding terminal toggle by class or aria
    const termToggle = page.locator('[class*="bg-\\[#111318\\]"] button').first();
    if (await termToggle.count() > 0) {
      await termToggle.click();
      await page.waitForTimeout(800);
      await page.screenshot({ path: `${SCREENSHOTS_DIR}/03_terminal_open.png` });
    }
  }

  // 4. Inception Deck view
  console.log('Taking screenshot: 04_inception_deck');
  const inceptionBtn = page.locator('button').filter({ hasText: /Inception Deck/i }).first();
  if (await inceptionBtn.count() > 0) {
    await inceptionBtn.click();
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/04_inception_deck.png` });
    const kanbanBtn = page.locator('button').filter({ hasText: /^Kanban$/ }).first();
    if (await kanbanBtn.count() > 0) {
      await kanbanBtn.click();
      await page.waitForTimeout(500);
    }
  }

  // 5. History Modal
  console.log('Taking screenshot: 05_history_modal');
  const historyBtn = page.locator('button').filter({ hasText: /履歴/ }).first();
  if (await historyBtn.count() > 0) {
    await historyBtn.click();
    await page.waitForTimeout(800);
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/05_history_modal.png` });
    // Close by clicking the X button inside the modal header
    const closeBtn = page.locator('.fixed.inset-0 button').last();
    if (await closeBtn.count() > 0) {
      // Find the header close button (X icon button)
      const headerClose = page.locator('.fixed.inset-0 .flex.items-center.justify-between button').first();
      if (await headerClose.count() > 0) {
        await headerClose.click();
      } else {
        await closeBtn.click();
      }
    }
    await page.waitForTimeout(600);
  }

  // 6. Settings Modal
  console.log('Taking screenshot: 06_settings_modal');
  const settingsBtn = page.locator('button[title="グローバル設定"]').first();
  if (await settingsBtn.count() > 0) {
    await settingsBtn.click();
    await page.waitForTimeout(800);
    await page.screenshot({ path: `${SCREENSHOTS_DIR}/06_settings_modal.png` });
    await page.keyboard.press('Escape');
    await page.waitForTimeout(300);
  }

  await browser.close();
  console.log('\nAll screenshots saved to', SCREENSHOTS_DIR);
}

takeScreenshots().catch(console.error);
