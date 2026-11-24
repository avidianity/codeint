# Framework root markers, convention entry points, and generated paths

Date checked: 2026-11-25.
Scope: Primary-source conventions for 17 target frameworks (15 families, splitting Vue 2 and Vue 3) across `typescript-javascript`, `php`, `python`, and `dart`.
Purpose: Reference for Map #435 Framework profile detection, `Orphan files` entry points, `Generated` `Source set` rules, and `Reference range` precedence.

## Cross-cutting synthesis

### Parent-child precedence for Reference ranges

When a child framework activates alongside its parent in one `Workspace package`, the child wins `Reference range` selection while unioning entry points and `Generated` paths.

| Child Framework profile | Parent Framework profile | Shared manifest signal | Precedence rule |
| ----------------------- | ------------------------ | ---------------------- | --------------- |
| Next.js | React | `next` with `react`, `react-dom` | Next.js overrides React |
| Nuxt | Vue 3 (or Vue 2 for Nuxt 2) | `nuxt` with `vue` | Nuxt overrides Vue |
| SvelteKit | Svelte | `@sveltejs/kit` with `svelte` | SvelteKit overrides Svelte |

No other pair among the 17 frameworks has a parent-child relationship.
Polyglot repositories (such as Laravel PHP plus Vue 3) activate one profile per owning Language plugin independently.

### Monorepo dependency hoisting rules

A monorepo root manifest must never activate a Framework profile on a child `Workspace package` without a local signal.

- **Node workspaces**: npm hoists shared dependencies to root `node_modules` ([npm][npm-ws]); pnpm isolates members unless hoisted via `pnpm-workspace.yaml` ([pnpm][pnpm-ws]).
  Require a local config (`next.config.*`, `nuxt.config.*`, `svelte.config.*`, `angular.json` project entry, `nest-cli.json` project entry) or the package in child `dependencies` (not only `peerDependencies`).
- **PHP Composer**: Composer links local packages via `path` repositories ([Composer][composer-path]).
  Require both the app package in local `composer.json` `require` and a local bootstrap marker (`artisan`, `bin/console` or `src/Kernel.php`, `bin/cake`, `spark`).
- **Python workspaces**: uv workspaces share root `uv.lock`, while each member declares its own `pyproject.toml` dependencies ([uv][uv-ws]).
  If dependencies live only at root, require a local marker (`manage.py`, `[tool.fastapi]` or CLI file importing `fastapi`, or `app.py`/`wsgi.py` importing `flask`).
- **Dart Pub workspaces**: Dart 3.6+ Pub workspaces share root `pubspec.lock` and `.dart_tool/package_config.json`, while each member keeps `pubspec.yaml` (`resolution: workspace`) and its own `dependencies` ([Pub][dart-ws]).
  Check each member's local `pubspec.yaml` for `dependencies.flutter.sdk: flutter`; never propagate Flutter from root.

### Verified isolation rules

- **Laravel vs Symfony**: `laravel/framework` 12.x directly requires 13 `symfony/*` packages (`console`, `error-handler`, `finder`, `http-foundation`, `http-kernel`, `mailer`, `mime`, `process`, `routing`, `uid`, `var-dumper`, `polyfill-*`), verified in [laravel/framework][laravel-fw-composer].
  It never requires `symfony/framework-bundle` or `symfony/flex`.
  Matching Symfony strictly on `symfony/framework-bundle` prevents Laravel apps and standalone Symfony component users from triggering Symfony.
- **Laravel vs `illuminate/*`**: `laravel/framework` replaces 37 `illuminate/*` packages ([laravel/framework][laravel-fw-composer]), while standalone PHP libraries depend on `illuminate/support` or `illuminate/database` alone ([Laravel Packages][laravel-packages]).
  Matching Laravel strictly on `laravel/framework` plus `artisan` or `bootstrap/app.php` excludes `illuminate/*` libraries.
- **Apps vs libraries**: UI libraries in React, Vue, Svelte, Angular, and NestJS list frameworks in `peerDependencies`, and Flutter plugins declare `flutter.plugin`.
  Require runtime `dependencies` (except Svelte and SvelteKit) plus app markers before enabling app-only entry rules.

---

## TypeScript and JavaScript (`typescript-javascript`)

### 1. React

- **Root markers**: `react` in `package.json` `dependencies` with a renderer (`react-dom` or `react-native`) or bundler entry (`src/main.{jsx,tsx}`, `src/index.{jsx,tsx}`), per [Creating a React App][react-create] and [Build from Scratch][react-scratch].
  Exclude `peerDependencies`-only UI libraries from app entry rules.
- **Monorepo**: Local `package.json` with `react` in `dependencies`, or local `index.html` referencing `src/main.{js,jsx,ts,tsx}` plus JSX/TSX files.
- **Entry points**: No file router; templates mount `src/main.{js,jsx,ts,tsx}` or `src/index.{js,jsx,ts,tsx}` from `index.html` or `public/index.html` ([Build from Scratch][react-scratch]).
- **Generated**: None React-specific (`dist/`, `build/` are bundler outputs).
- **Hierarchy**: Parent of Next.js.
- **Risks**: Libraries export `src/index.{ts,tsx}` via `exports`/`main` without `react-dom`; never mark `src/App.{jsx,tsx}` as an entry point.

### 2. Next.js

- **Root markers**: `next` in `package.json` `dependencies` with `next.config.{js,mjs,cjs,ts}` ([App structure][nextjs-app]); shared packages importing `next/link` use `peerDependencies` without `next.config.*` or route folders.
- **Monorepo**: Local `next.config.*`, `{,src/}app/layout.{js,jsx,ts,tsx}`, or `{,src/}pages/_app.{js,jsx,ts,tsx}`, plus `next` in child `dependencies`.
- **Entry points**: Match at root or under `src/` ([App][nextjs-app]; [Pages][nextjs-pages]; [Conventions][nextjs-conv]; [proxy.js][nextjs-proxy]):
  - Root/src: `next.config.{js,mjs,cjs,ts}`, `proxy.{js,ts}` (v16+), `middleware.{js,ts}` (v12-15), `instrumentation{,-client}.{js,ts}`, `mdx-components.{js,jsx,ts,tsx}`.
  - App Router (`{,src/}app/**`): `{page,layout,template,loading,error,global-error,not-found,forbidden,unauthorized,default}.{js,jsx,ts,tsx}`, `route.{js,ts}`, metadata `{sitemap,robots,manifest}.{js,ts}` and `{icon,apple-icon,opengraph-image,twitter-image}.{js,jsx,ts,tsx}`.
  - Pages Router (`{,src/}pages/**`): every `.{js,jsx,ts,tsx}` file (`_app`, `_document`, `_error`, `404`, `500`, `pages/api/**`).
- **Generated**: `.next/` (build cache and `.next/types/`), `next-env.d.ts`, and `out/` for static exports ([Static Exports][nextjs-static]).
- **Hierarchy**: Child of React; Next.js wins `Reference range` precedence.
- **Risks**: App Router allows colocated helpers under `app/` (`_components/`), so match only named convention files; custom `pageExtensions` needs config overrides.

### 3. Vue 2

- **Root markers**: `vue` in `package.json` `dependencies` matching major `2` (`^2.`, `~2.`, `2.`), optionally with `vue-template-compiler` or `@vue/cli-service` and `vue.config.{js,cjs,mjs}` ([Vue 2 Install][vue2-install]; [Vue CLI Config][vue-cli-config]).
- **Monorepo**: Local `package.json` with `vue` `2.x` in `dependencies` or local `vue.config.{js,cjs,mjs}` with `.vue` files.
- **Entry points**: `src/main.{js,ts}` and `vue.config.{js,cjs,mjs}` (multi-page apps list entries under `pages`, [Vue CLI Config][vue-cli-config]).
- **Generated**: `dist/` (default Vue CLI `outputDir`, [Vue CLI Assets][vue-cli-assets]).
- **Hierarchy**: Parent of Nuxt 2 (`nuxt` `2.x`); distinct from Vue 3.
- **Risks**: Vue 2.7 supports Vite (`@vitejs/plugin-vue2`), so separate Vue 2 from Vue 3 via the `vue` version range, falling back to `vue-template-compiler` vs `@vue/compiler-sfc` for `catalog:`/`workspace:*`.

### 4. Vue 3

- **Root markers**: `vue` in `package.json` `dependencies` matching major `3` (`>=3`), typically with `@vitejs/plugin-vue` and `vite.config.{js,mjs,ts}` ([Vue Quick Start][vue3-start]; [Vue Tooling][vue3-tooling]).
- **Monorepo**: Local `package.json` with `vue` `3.x` in `dependencies` or local `vite.config.{js,mjs,ts}` referencing `@vitejs/plugin-vue` with `.vue` files.
- **Entry points**: `src/main.{js,ts}` and `vite.config.{js,mjs,ts}` ([Vue Quick Start][vue3-start]).
- **Generated**: `dist/` (Vite output) plus optional `auto-imports.d.ts` and `components.d.ts`.
- **Hierarchy**: Parent of Nuxt (v3 and v4).
- **Risks**: Component libraries put `vue` in `peerDependencies` without `src/main.ts`; third-party file routers (`unplugin-vue-router`) belong in user `entry_points`.

### 5. Nuxt

- **Root markers**: `nuxt` in `package.json` `dependencies` or `devDependencies` paired with `nuxt.config.{ts,js,mjs,cjs}` ([Nuxt 4][nuxt4-dir]).
  Nuxt modules put `@nuxt/kit` in `dependencies` and keep `nuxt.config.ts` only inside `playground/`.
- **Monorepo**: Local `nuxt.config.{ts,js,mjs,cjs}` in the child package root ([Nuxt 4][nuxt4-dir]).
- **Entry points**: Nuxt 4 moves client folders under `app/` while Nuxt 3 keeps them at root; both keep `server/` and `modules/` at root ([Nuxt 4][nuxt4-dir]; [Nuxt 3][nuxt3-dir]; [Nuxt server/][nuxt4-server]).
  Match `{,app/}` prefixes:
  - Root/app: `nuxt.config.{ts,js,mjs,cjs}`, `{,app/}{app,error}.vue`, `{,app/}{app.config,router.options}.{ts,js}`.
  - Client dirs: `{,app/}pages/**/*.{vue,js,ts}`, `{,app/}{layouts,components}/**/*.vue`, `{,app/}{middleware,plugins,composables,utils}/**/*.{js,ts}`.
  - Server/shared: `server/{api,routes,middleware,plugins,utils}/**/*.{js,ts}`, `modules/**/*.{js,ts}`, `shared/utils/**/*.{js,ts}`.
- **Generated**: `.nuxt/` ([Nuxt .nuxt][nuxt4-nuxt]), `.output/` ([Nuxt .output][nuxt4-output]), and `dist/` for Nuxt 2 ([Nuxt 2][nuxt2-nuxt]).
- **Hierarchy**: Child of Vue 3 (v3/v4) or Vue 2 (v2); Nuxt wins `Reference range` precedence.
- **Risks**: Auto-imported `components/`, `composables/`, and `utils/` lack `import` lines; marking them as entries avoids false `Orphan files` while hiding unused auto-imported helpers.

### 6. Svelte

- **Root markers**: `svelte` in `package.json` `dependencies` or `devDependencies` (compiler templates often list `svelte` in `devDependencies`), without `@sveltejs/kit` ([Svelte Getting Started][svelte-start]).
- **Monorepo**: Local `package.json` declaring `svelte`, or local `svelte.config.{js,mjs,cjs,ts}` with `.svelte` files and no `@sveltejs/kit`.
- **Entry points**: `src/main.{js,ts}`, `svelte.config.{js,mjs,cjs,ts}`, `vite.config.{js,mjs,ts}` ([Svelte Getting Started][svelte-start]).
- **Generated**: None Svelte-specific; Vite outputs to `dist/`.
- **Hierarchy**: Parent of SvelteKit.
- **Risks**: Because `svelte` legitimately sits in `devDependencies`, separate libraries from apps via `exports`/`svelte` fields and `index.html`.

### 7. SvelteKit

- **Root markers**: `@sveltejs/kit` in `package.json` `dependencies` or `devDependencies` with `svelte.config.{js,mjs,cjs,ts}` or `@sveltejs/kit/vite` in `vite.config.*` ([SvelteKit Structure][kit-struct]; [@sveltejs/kit/vite][kit-vite]).
- **Monorepo**: Local `svelte.config.{js,mjs,cjs,ts}` or local `src/routes/` containing `+page.*`, `+layout.*`, or `+server.*`.
- **Entry points**: Verified from [Structure][kit-struct], [Routing][kit-routing], and [Hooks][kit-hooks]:
  - Config: `{svelte,vite}.config.{js,mjs,cjs,ts}`.
  - Routes (`src/routes/**`): `+{page,layout}.svelte`, `+{page,layout}{,.server}.{js,ts}`, `+server.{js,ts}`, `+error.svelte` (including `@` resets like `+page@foo.svelte`).
  - App-wide: `src/hooks{,.server,.client}.{js,ts}`, `src/instrumentation.server.{js,ts}`, `src/params{,/*}.{js,ts}`, `src/service-worker{,/index}.{js,ts}`.
- **Generated**: `.svelte-kit/` (manifests and `./$types`, [Structure][kit-struct]) and `build/` (default output for `adapter-node` and `adapter-static`, [adapter-node][kit-node]).
- **Hierarchy**: Child of Svelte; SvelteKit wins `Reference range` precedence.
- **Risks**: Colocated helpers inside `src/routes/**` lack `+`, so match only `+`-prefixed route files; `svelte.config.js` can relocate `kit.files.*`.

### 8. Angular

- **Root markers**: `@angular/core` in `package.json` `dependencies` paired with `angular.json` (or `project.json` in Nx) ([Angular Structure][ng-struct]; [Workspace Config][ng-ws]).
  Libraries put `@angular/core` in `peerDependencies` and set `"projectType": "library"`.
- **Monorepo**: Root `angular.json` maps every sub-project under `projects.<name>` with `root`, `sourceRoot`, `projectType` (`"application"` vs `"library"`), and build entry options `browser`/`main`/`server` ([Workspace Config][ng-ws]).
- **Entry points**: `src/main.ts`, `src/main.server.ts`, `src/server.ts` ([Angular SSR][ng-ssr]), configured `architect.build.options.{browser,main,server,polyfills}` paths, and `public-api.ts` for libraries.
- **Generated**: `.angular/` (`.angular/cache/`, [Workspace Config][ng-ws]) and `dist/`.
- **Hierarchy**: Standalone.
- **Risks**: Multi-project Angular workspaces place apps under `projects/<app>/` without per-project `package.json`; workspace discovery must read `angular.json`.

### 9. NestJS

- **Root markers**: `@nestjs/core` in `package.json` `dependencies` with `nest-cli.json` ([First Steps][nest-first]; [starter package.json][nest-pkg]).
  NestJS libraries peer-depend on `@nestjs/common` without `@nestjs/core`, preventing library false positives.
- **Monorepo**: Root `nest-cli.json` (`"monorepo": true`) maps sub-projects under `projects.<name>` with `type` (`"application"` vs `"library"`), `root`, `sourceRoot`, and `entryFile` ([NestJS Monorepo][nest-mono]).
  In npm/pnpm workspaces, require local `@nestjs/core` in `dependencies` or local `nest-cli.json`.
- **Entry points**: `src/main.ts` (or `<sourceRoot>/<entryFile>.ts`, defaulting `entryFile` to `main`, [nest-cli.json][nest-cli]; [Monorepo][nest-mono]) and `libs/<lib>/src/index.ts`; `@Module({...})` imports controllers statically.
- **Generated**: `dist/` (`nest build` output, [First Steps][nest-first]).
- **Hierarchy**: Standalone.
- **Risks**: TypeORM/MikroORM entities loaded by string globs (`dist/**/*.entity.js`) have no static imports; native NestJS monorepos share one root `package.json`.

---

## PHP (`php`)

### 10. Laravel

- **Root markers**: `laravel/framework` in `composer.json` `require` ([laravel/laravel][laravel-app-composer]) with `artisan` and `bootstrap/app.php` ([Structure][laravel-struct]).
  Never match `illuminate/*` alone ([laravel/framework][laravel-fw-composer]; [Laravel Packages][laravel-packages]); legacy Lumen apps use `laravel/lumen-framework`.
- **Monorepo**: Local `composer.json` requiring `laravel/framework` plus local `artisan` or `bootstrap/app.php`.
  Packages requiring `laravel/framework` only in `require-dev` (Orchestra Testbench) lack `artisan` and do not match.
- **Entry points**: Verified from [Structure][laravel-struct], [Providers][laravel-providers], [Artisan][laravel-artisan], [Events][laravel-events], and [Authorization][laravel-auth]:
  - Core: `public/index.php`, `artisan`, `bootstrap/{app,providers}.php`, `config/*.php`, `routes/*.php`.
  - Auto-discovered: `app/Console/Commands/**/*.php`, `app/{Listeners,Policies,Models/Policies,Providers}/**/*.php`.
  - Database: `database/migrations/*.php`, `database/{seeders,factories}/**/*.php`.
- **Generated**: `bootstrap/cache/`, `storage/framework/` (compiled Blade views, cache, sessions), `public/build/`, `public/hot` ([.gitignore][laravel-gitignore]; [Structure][laravel-struct]).
- **Hierarchy**: Standalone.
- **Risks**: `laravel/framework` requires 13 `symfony/*` packages (see isolation rules above); legacy string routes (`'UserController@index'`) lack PHP `use` imports.

### 11. Symfony

- **Root markers**: `symfony/framework-bundle` in `composer.json` `require` (or `flex-require` in an uninitialized skeleton) with `config/bundles.php`, `src/Kernel.php`, `bin/console`, or `symfony.lock` ([Setup][sym-setup]; [Flex][sym-flex]; [symfony/skeleton][sym-skeleton]).
  Never match standalone `symfony/*` components ([Symfony Packages][sym-packages]).
- **Monorepo**: Local `composer.json` requiring `symfony/framework-bundle` (excluding `type: "symfony-bundle"`) plus local `config/bundles.php`, `src/Kernel.php`, or `symfony.lock`.
- **Entry points**: Verified from [FrameworkBundle recipe][sym-recipe] and [Service Container][sym-di]:
  - Core: `public/index.php`, `bin/console`, `src/Kernel.php`, `config/{bundles,preload,routes,services}.php`.
  - Autoconfigured (`config/services.yaml` scans `../src/`): `src/{Controller,Command,EventListener,EventSubscriber,Twig,Security,Form,DataFixtures}/**/*.php`, `migrations/*.php`.
- **Generated**: `var/cache/`, `var/log/`, `public/bundles/`, `public/build/` ([FrameworkBundle recipe][sym-recipe]).
- **Hierarchy**: Standalone.
- **Risks**: `autoconfigure: true` registers attribute-tagged classes (`#[Route]`, `#[AsCommand]`, `#[AsMessageHandler]`) in arbitrary domain directories (`src/Domain/*/CommandHandler/`), which folder patterns alone may miss.

### 12. CakePHP

- **Root markers**: `cakephp/cakephp` in `composer.json` `require` ([cakephp/app][cake-composer]) with `src/Application.php`, `bin/cake`, `config/routes.php`, and `webroot/index.php` ([Conventions][cake-conv]).
  Exclude standalone components (`cakephp/orm`, `cakephp/database`, `cakephp/chronos`).
- **Monorepo**: Local `composer.json` requiring `cakephp/cakephp` (excluding `type: "cakephp-plugin"`) plus local `src/Application.php` or `bin/cake`.
- **Entry points**: Verified from [Conventions][cake-conv], [Routing][cake-routing], and [Console][cake-console]:
  - Core: `webroot/index.php`, `bin/cake`, `bin/cake.php`, `src/Application.php`, `config/{bootstrap,bootstrap_cli,routes,paths,app}.php`.
  - Convention classes: `src/Controller/**/*Controller.php`, `src/Command/**/*Command.php`, `src/Model/Table/**/*Table.php`, `src/Model/Entity/**/*.php`, `src/View/Helper/**/*Helper.php`, `src/View/Cell/**/*Cell.php`, `src/Mailer/**/*Mailer.php`, `templates/**/*.php`, `config/{Migrations,Seeds}/*.php`.
- **Generated**: `tmp/` (cache, metadata, sessions) and `logs/` ([.gitignore][cake-gitignore]; [Conventions][cake-conv]).
- **Hierarchy**: Standalone.
- **Risks**: Controllers, tables (`fetchTable('Articles')`), helpers, and cells load by naming convention string rather than `use` imports; plugins under `plugins/` lack `webroot/index.php` and `bin/cake`.

### 13. CodeIgniter (v4)

- **Root markers**: `codeigniter4/framework` in `composer.json` `require` ([appstarter][ci4-composer]) with `spark`, `app/Config/Routes.php`, and `public/index.php` ([Structure][ci4-struct]).
  For manual non-Composer installs, require `spark`, `app/Config/Paths.php`, and `system/Boot.php`.
- **Monorepo**: Local `composer.json` requiring `codeigniter4/framework` plus local `spark` or `app/Config/Routes.php`.
- **Entry points**: Verified from [Structure][ci4-struct], [Routing][ci4-routing], and [Spark Commands][ci4-commands]:
  - Core: `public/index.php`, `spark`, `app/Config/*.php`, `app/Config/Boot/*.php`.
  - Convention dirs: `app/{Controllers,Commands,Filters,Views}/**/*.php`, `app/Database/Migrations/*.php`, `app/Database/Seeds/**/*.php`, `app/Helpers/*_helper.php`.
- **Generated**: `writable/` (`cache/`, `logs/`, `session/`, `debugbar/`, `uploads/`, [.gitignore][ci4-gitignore]); classify checked-in `system/` under `Vendored`.
- **Hierarchy**: Standalone.
- **Risks**: Routes (`'Home::index'`), `model()`, and `helper()` use string lookups rather than `use` statements; requiring `codeigniter4/framework` or `spark` excludes CodeIgniter 3 (`application/` + root `index.php`).

---

## Python (`python`)

### 14. Django

- **Root markers**: `django` (PEP 503 case-insensitive) in `pyproject.toml`, `requirements*.txt`, `setup.cfg`, or `Pipfile`, paired with `manage.py` ([Tutorial 01][dj-tut]; [Applications][dj-apps]).
  Requiring `manage.py` (or a `settings.py` + `wsgi.py`/`asgi.py` triad) excludes reusable Django libraries.
- **Monorepo**: Local `manage.py` inside the child `Workspace package` or local `pyproject.toml` declaring `django` with a local settings module.
- **Entry points**: Verified from [Applications][dj-apps], [Management Commands][dj-mgmt], [Admin][dj-admin], [Migrations][dj-mig], and [Template Tags][dj-tags]:
  - Project: `manage.py`, `**/{wsgi,asgi,settings,urls}.py`, `**/settings/*.py`.
  - App-discovered: `**/{apps,models,admin,checks,signals}.py`, `**/models/*.py`, `**/management/commands/[!_]*.py`, `**/templatetags/[!_]*.py`, `**/migrations/*.py`.
- **Generated**: `**/migrations/[0-9][0-9][0-9][0-9]_*.py` (`makemigrations`, [Migrations][dj-mig]) and collected static output (`staticfiles/` or `STATIC_ROOT`, [Static files][dj-static]).
- **Hierarchy**: Standalone.
- **Risks**: Teams occasionally hand-write `RunPython` data migrations inside numbered migration files; default them to `Generated` with `source_sets` overrides, and normalize PEP 503 names (`Django` vs `django`).

### 15. FastAPI

- **Root markers**: `fastapi` (including extras like `fastapi[standard]`) in `pyproject.toml`, `requirements*.txt`, or `Pipfile` ([FastAPI CLI][fa-cli]); never match `starlette` or `uvicorn` alone.
- **Monorepo**: Local `pyproject.toml` declaring `fastapi` (or `[tool.fastapi] entrypoint`), or one of FastAPI CLI's six default files importing `fastapi`.
- **Entry points**: Verified from [FastAPI CLI][fa-cli], [Bigger Applications][fa-big], and `fastapi-cli` [discover.py][fa-discover]:
  - Default CLI paths: `main.py`, `app.py`, `api.py`, `app/main.py`, `app/app.py`, `app/api.py`.
  - Explicit manifest entry: `[tool.fastapi] entrypoint` in `pyproject.toml` (`app.main:app` maps to `app/main.py`).
  - `APIRouter` modules are imported statically via `app.include_router(...)`.
- **Generated**: None FastAPI-specific (`__pycache__/`, `.venv/`, `.pytest_cache/` belong to base Python).
- **Hierarchy**: Standalone.
- **Risks**: Services started via `uvicorn src.pkg.api:create_app --factory` without `[tool.fastapi]` need user `entry_points`; Alembic (`alembic/versions/*.py`) is separate from FastAPI.

### 16. Flask

- **Root markers**: `flask` (normalized from `Flask`) in `pyproject.toml`, `requirements*.txt`, or `Pipfile` ([Flask CLI][fl-cli]); never match `werkzeug`, `jinja2`, or `click` alone.
- **Monorepo**: Local `pyproject.toml` declaring `flask`, or local `app.py`, `wsgi.py`, `app/__init__.py`, or `wsgi/__init__.py` importing `flask` (or a package `__init__.py` defining `create_app` / `make_app`).
- **Entry points**: Verified from [Flask CLI][fl-cli], [Project Layout][fl-layout], and [App Factories][fl-factory]:
  - Default CLI targets: `app.py`, `wsgi.py`, `app/__init__.py`, `wsgi/__init__.py` (searching for `app`, `application`, `create_app`, `make_app`).
  - Factory fallback: top-level `<pkg>/__init__.py` defining `create_app` or `make_app` when `app.py` and `wsgi.py` are absent.
- **Generated**: `instance/` (local runtime config and SQLite folder, [Project Layout][fl-layout]); no compiled source output.
- **Hierarchy**: Standalone.
- **Risks**: Flask and FastAPI both recognize `app.py` and can coexist via `WSGIMiddleware`; distinct package names isolate detection, while `Reference range` needs a tie-break rule if both activate together.

---

## Dart (`dart`)

### 17. Flutter

- **Root markers**: `pubspec.yaml` declaring `dependencies.flutter.sdk: flutter` ([Flutter pubspec][flut-pubspec]; [app template][flut-app-pubspec]).
  Separate Flutter apps from publishable packages or plugins by checking `lib/main.dart` presence and absence of `flutter.plugin` ([package template][flut-pkg-pubspec]; [plugin template][flut-plug-pubspec]).
- **Monorepo**: Dart Pub Workspaces keep a local `pubspec.yaml` (`resolution: workspace`) in every member ([Pub workspaces][dart-ws]), and each Flutter member declares `dependencies.flutter.sdk: flutter` locally.
- **Entry points**:
  - Apps: `lib/main.dart` and flavor entries `lib/main_*.dart`.
  - Packages/plugins: `lib/<package_name>.dart` ([Developing packages][flut-dev-pkg]).
  - Integration tests (`Test` `Source set`): `integration_test/**/*_test.dart`.
- **Generated**: Verified from [app .gitignore.tmpl][flut-gitignore], [JSON serialization][flut-json], and [Internationalization][flut-l10n]:
  - SDK/build caches: `.dart_tool/`, `build/`, `.widget_preview/`.
  - Flutter-generated Dart: `lib/generated_plugin_registrant.dart`, `lib/l10n/app_localizations*.dart` (or `l10n.yaml` `output-dir`), `.dart_tool/flutter_gen/**`.
  - Code-gen parts: `**/*.{g,freezed,gr,mocks,pb,pbenum,pbjson,pbserver}.dart`.
- **Hierarchy**: Standalone within `dart` (framework profile over base Dart).
- **Risks**: Apps, packages, and plugins all declare `sdk: flutter` and `flutter:`; gate `lib/main.dart` on app presence while keeping `lib/<package_name>.dart` for workspace UI packages.
  Flutter 3.38+ apps also add standalone `material_ui`/`cupertino_ui`, while `sdk: flutter` remains invariant.

---

## WIT and Framework profile design caveats

1. **Two-key activation**: Require an app-level manifest package (allowing `devDependencies` only for Svelte and SvelteKit) plus a local marker whenever a manifest is hoisted from root.
2. **Non-`package.json` sub-projects**: Angular (`angular.json`) and NestJS monorepos (`nest-cli.json`) declare multiple apps under one root `package.json`; `typescript-javascript` workspace discovery must read both files.
3. **Version-sensitive profiles**: Vue 2 vs Vue 3 requires `vue` semver inspection (with compiler-package fallback); Nuxt needs `{,app/}` prefixes for v3 and v4.
4. **Dynamic dispatch limits**: Symfony attributes, Laravel string routes, CodeIgniter Auto Routing, and NestJS ORM globs bypass static imports; folder conventions handle defaults and `entry_points` handles custom layouts.
5. **Co-installed unrelated frameworks**: Parent-child pairs have fixed `Reference range` precedence; unrelated co-installed frameworks in one `Workspace package` (such as Flask inside FastAPI) need a WIT ordering or multi-profile rule.
6. **Committed generated files**: `next-env.d.ts`, Django numbered migrations, and Flutter `*.{g,freezed}.dart` and `app_localizations*.dart` are often tracked in Git; classifying them as `Generated` keeps `Erosion`, `Duplication`, and `Verbosity` accurate.

## Primary sources (checked 2026-11-25)

[npm-ws]: https://docs.npmjs.com/cli/v11/using-npm/workspaces
[pnpm-ws]: https://pnpm.io/workspaces
[composer-path]: https://getcomposer.org/doc/05-repositories.md
[uv-ws]: https://docs.astral.sh/uv/concepts/projects/workspaces/
[dart-ws]: https://dart.dev/tools/pub/workspaces
[react-create]: https://react.dev/learn/creating-a-react-app
[react-scratch]: https://react.dev/learn/build-a-react-app-from-scratch
[nextjs-app]: https://nextjs.org/docs/app/getting-started/project-structure
[nextjs-pages]: https://nextjs.org/docs/pages/getting-started/project-structure
[nextjs-conv]: https://nextjs.org/docs/app/api-reference/file-conventions
[nextjs-proxy]: https://nextjs.org/docs/app/api-reference/file-conventions/proxy
[nextjs-static]: https://nextjs.org/docs/app/guides/static-exports
[vue2-install]: https://v2.vuejs.org/v2/guide/installation.html
[vue-cli-config]: https://cli.vuejs.org/config/
[vue-cli-assets]: https://cli.vuejs.org/guide/html-and-static-assets.html
[vue3-start]: https://vuejs.org/guide/quick-start.html
[vue3-tooling]: https://vuejs.org/guide/scaling-up/tooling.html
[nuxt4-dir]: https://nuxt.com/docs/4.x/directory-structure
[nuxt3-dir]: https://nuxt.com/docs/3.x/directory-structure
[nuxt4-server]: https://nuxt.com/docs/4.x/directory-structure/server
[nuxt4-nuxt]: https://nuxt.com/docs/4.x/directory-structure/nuxt
[nuxt4-output]: https://nuxt.com/docs/4.x/directory-structure/output
[nuxt2-nuxt]: https://nuxt.com/docs/2.x/directory-structure/nuxt
[svelte-start]: https://svelte.dev/docs/svelte/getting-started
[kit-struct]: https://svelte.dev/docs/kit/project-structure
[kit-vite]: https://svelte.dev/docs/kit/@sveltejs-kit-vite
[kit-routing]: https://svelte.dev/docs/kit/routing
[kit-hooks]: https://svelte.dev/docs/kit/hooks
[kit-node]: https://svelte.dev/docs/kit/adapter-node
[ng-struct]: https://angular.dev/reference/configs/file-structure
[ng-ws]: https://angular.dev/reference/configs/workspace-config
[ng-ssr]: https://angular.dev/guide/ssr
[nest-first]: https://docs.nestjs.com/first-steps
[nest-mono]: https://docs.nestjs.com/cli/monorepo
[nest-pkg]: https://raw.githubusercontent.com/nestjs/typescript-starter/master/package.json
[nest-cli]: https://raw.githubusercontent.com/nestjs/typescript-starter/master/nest-cli.json
[laravel-app-composer]: https://raw.githubusercontent.com/laravel/laravel/12.x/composer.json
[laravel-fw-composer]: https://raw.githubusercontent.com/laravel/framework/12.x/composer.json
[laravel-gitignore]: https://raw.githubusercontent.com/laravel/laravel/12.x/.gitignore
[laravel-struct]: https://laravel.com/framework/docs/13.x/structure
[laravel-providers]: https://laravel.com/framework/docs/13.x/providers
[laravel-artisan]: https://laravel.com/framework/docs/13.x/artisan
[laravel-events]: https://laravel.com/framework/docs/13.x/events
[laravel-auth]: https://laravel.com/framework/docs/13.x/authorization
[laravel-packages]: https://laravel.com/framework/docs/13.x/packages
[sym-setup]: https://symfony.com/doc/current/setup.html
[sym-flex]: https://symfony.com/doc/current/setup/flex.html
[sym-skeleton]: https://raw.githubusercontent.com/symfony/skeleton/8.0/composer.json
[sym-recipe]: https://raw.githubusercontent.com/symfony/recipes/main/symfony/framework-bundle/7.0/manifest.json
[sym-packages]: https://symfony.com/packages
[sym-di]: https://symfony.com/doc/current/service_container.html
[cake-composer]: https://raw.githubusercontent.com/cakephp/app/5.x/composer.json
[cake-gitignore]: https://raw.githubusercontent.com/cakephp/app/5.x/.gitignore
[cake-conv]: https://book.cakephp.org/5.x/intro/conventions.html
[cake-routing]: https://book.cakephp.org/5.x/development/routing.html
[cake-console]: https://book.cakephp.org/5.x/console-commands.html
[ci4-composer]: https://raw.githubusercontent.com/codeigniter4/appstarter/master/composer.json
[ci4-gitignore]: https://raw.githubusercontent.com/codeigniter4/appstarter/master/.gitignore
[ci4-struct]: https://codeigniter.com/user_guide/concepts/structure.html
[ci4-routing]: https://codeigniter.com/user_guide/incoming/routing.html
[ci4-commands]: https://codeigniter.com/user_guide/cli/cli_commands.html
[dj-tut]: https://docs.djangoproject.com/en/6.1/intro/tutorial01/
[dj-apps]: https://docs.djangoproject.com/en/6.1/ref/applications/
[dj-mgmt]: https://docs.djangoproject.com/en/6.1/howto/custom-management-commands/
[dj-admin]: https://docs.djangoproject.com/en/6.1/ref/contrib/admin/
[dj-mig]: https://docs.djangoproject.com/en/6.1/topics/migrations/
[dj-tags]: https://docs.djangoproject.com/en/6.1/howto/custom-template-tags/
[dj-static]: https://docs.djangoproject.com/en/6.1/howto/static-files/
[fa-cli]: https://fastapi.tiangolo.com/fastapi-cli/
[fa-big]: https://fastapi.tiangolo.com/tutorial/bigger-applications/
[fa-discover]: https://raw.githubusercontent.com/fastapi/fastapi-cli/main/src/fastapi_cli/discover.py
[fl-cli]: https://flask.palletsprojects.com/en/stable/cli/
[fl-layout]: https://flask.palletsprojects.com/en/stable/tutorial/layout/
[fl-factory]: https://flask.palletsprojects.com/en/stable/patterns/appfactories/
[flut-pubspec]: https://docs.flutter.dev/tools/pubspec
[flut-dev-pkg]: https://docs.flutter.dev/packages-and-plugins/developing-packages
[flut-json]: https://docs.flutter.dev/data-and-backend/serialization/json
[flut-l10n]: https://docs.flutter.dev/ui/internationalization
[flut-app-pubspec]: https://raw.githubusercontent.com/flutter/flutter/master/packages/flutter_tools/templates/app/pubspec.yaml.tmpl
[flut-pkg-pubspec]: https://raw.githubusercontent.com/flutter/flutter/master/packages/flutter_tools/templates/package/pubspec.yaml.tmpl
[flut-plug-pubspec]: https://raw.githubusercontent.com/flutter/flutter/master/packages/flutter_tools/templates/plugin_shared/pubspec.yaml.tmpl
[flut-gitignore]: https://raw.githubusercontent.com/flutter/flutter/master/packages/flutter_tools/templates/app/.gitignore.tmpl
