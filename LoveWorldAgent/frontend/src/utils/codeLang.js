// Monaco 编辑器支持的内置语言标识集合（monaco-editor@0.57.0）
export const MONACO_LANGS = new Set([
    'abap', 'aes', 'apex', 'azcli', 'bat', 'bicep', 'c', 'cameligo', 'clojure', 'coffeescript', // 通用及脚本语言标识
    'cpp', 'csharp', 'csp', 'css', 'cypher', 'dart', 'dockerfile', 'ecl', 'elixir', 'flow9', // 系统与前端扩展语言
    'freemarker2', 'fsharp', 'go', 'graphql', 'handlebars', 'hcl', 'html', 'ini', 'java', // 编译型与配置模版语言
    'javascript', 'json', 'julia', 'kotlin', 'less', 'lexon', 'liquid', 'lua', 'm3', 'markdown', // 动态脚本与文档语言
    'mdx', 'mips', 'msdax', 'mysql', 'objective-c', 'pascal', 'pascaligo', 'perl', 'pgsql', // 查询与工程语言
    'php', 'pla', 'plaintext', 'postiats', 'powerquery', 'powershell', 'proto', 'pug', 'python', // 服务端与终端脚本语言
    'qsharp', 'r', 'razor', 'redis', 'redshift', 'restructuredtext', 'ruby', 'rust', 'sb', // 数据分析与安全语言
    'scala', 'scheme', 'scss', 'shell', 'sol', 'sparql', 'sql', 'st', 'swift', 'systemverilog', // 硬件与智能合约语言
    'tcl', 'twig', 'typescript', 'typespec', 'vb', 'verilog', 'wgsl', 'xml', 'yaml', // 类型定义与图形着色语言
])
const LANG_ALIASES = { // 常见代码块别名及文件后缀映射表
    text: 'plaintext', txt: 'plaintext', plain: 'plaintext', plaintext: 'plaintext', // 纯文本别名
    log: 'plaintext', none: 'plaintext', nohighlight: 'plaintext', diff: 'plaintext', patch: 'plaintext', // 日志与差异文本
    csv: 'plaintext', tsv: 'plaintext', // 分隔符表格纯文本
    rs: 'rust', rust: 'rust', rlib: 'rust', // Rust 源码及库文件
    ps: 'powershell', ps1: 'powershell', psm1: 'powershell', psd1: 'powershell', // PowerShell 脚本与模块
    pwsh: 'powershell', powershell: 'powershell', posh: 'powershell', // PowerShell 交互命令行
    bat: 'bat', cmd: 'bat', batch: 'bat', dos: 'bat', // Windows 批处理脚本
    sh: 'shell', bash: 'shell', zsh: 'shell', ash: 'shell', ksh: 'shell', csh: 'shell', // 类 Unix Shell 变体
    fish: 'shell', shell: 'shell', shellscript: 'shell', console: 'shell', terminal: 'shell', // 终端会话脚本别名
    makefile: 'shell', // 构建脚本沿用 Shell 高亮
    c: 'c', h: 'c', // C 语言源码及头文件
    'c++': 'cpp', cpp: 'cpp', cxx: 'cpp', cc: 'cpp', hpp: 'cpp', hh: 'cpp', hxx: 'cpp', 'h++': 'cpp', inl: 'cpp', // C++ 源码与头文件
    cs: 'csharp', 'c#': 'csharp', csharp: 'csharp', csx: 'csharp', cake: 'csharp', // C# 与脚本文件
    fs: 'fsharp', 'f#': 'fsharp', fsharp: 'fsharp', fsx: 'fsharp', fsi: 'fsharp', fsscript: 'fsharp', ml: 'fsharp', mli: 'fsharp', // F# 语言系列
    go: 'go', golang: 'go', // Go 语言后缀
    java: 'java', jav: 'java', // Java 源码后缀
    kt: 'kotlin', kts: 'kotlin', kotlin: 'kotlin', // Kotlin 源码及 Gradle 脚本
    swift: 'swift', // Swift 语言后缀
    rb: 'ruby', rbx: 'ruby', rjs: 'ruby', ruby: 'ruby', gemspec: 'ruby', rake: 'ruby', // Ruby 源码及构建描述
    php: 'php', php4: 'php', php5: 'php', phtml: 'php', // PHP 脚本及模版
    lua: 'lua', // Lua 脚本
    r: 'r', rlang: 'r', rmd: 'r', // R 语言分析脚本
    pl: 'perl', pm: 'perl', perl: 'perl', // Perl 脚本与模块
    py: 'python', pyw: 'python', pyi: 'python', python: 'python', python3: 'python', gyp: 'python', // Python 源码及存根文件
    js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'javascript', node: 'javascript', javascript: 'javascript', // JavaScript 与 JSX
    ts: 'typescript', mts: 'typescript', cts: 'typescript', tsx: 'typescript', typescript: 'typescript', // TypeScript 与 TSX
    vue: 'html', svelte: 'html', astro: 'html', htm: 'html', html: 'html', xhtml: 'html', jsp: 'html', asp: 'html', aspx: 'html', // 单文件组件及 HTML 模版
    css: 'css', // CSS 样式表
    scss: 'scss', sass: 'scss', // Sass 预处理器样式
    less: 'less', // Less 预处理器样式
    json: 'json', jsonc: 'json', json5: 'json', jsonl: 'json', ndjson: 'json', geojson: 'json', webmanifest: 'json', // JSON 系列数据格式
    yml: 'yaml', yaml: 'yaml', // YAML 配置格式
    toml: 'ini', ini: 'ini', conf: 'ini', cfg: 'ini', cnf: 'ini', properties: 'ini', env: 'ini', dotenv: 'ini', editorconfig: 'ini', gitconfig: 'ini', // 键值对配置文件
    xml: 'xml', svg: 'xml', xaml: 'xml', csproj: 'xml', fsproj: 'xml', vbproj: 'xml', props: 'xml', targets: 'xml', // XML 及 .NET 工程配置
    plist: 'xml', xsl: 'xml', xslt: 'xml', xsd: 'xml', dtd: 'xml', pom: 'xml', // 派生 XML 描述文件
    docker: 'dockerfile', dockerfile: 'dockerfile', containerfile: 'dockerfile', // 容器构建定义文件
    tf: 'hcl', tfvars: 'hcl', terraform: 'hcl', hcl: 'hcl', // Terraform 配置语言
    bicep: 'bicep', // Azure Bicep 声明文件
    azcli: 'azcli', // Azure CLI 脚本
    md: 'markdown', markdown: 'markdown', mdown: 'markdown', mkd: 'markdown', mkdn: 'markdown', // Markdown 文档后缀
    mdx: 'mdx', // MDX 组件化文档
    rst: 'restructuredtext', restructuredtext: 'restructuredtext', // reStructuredText 文档
    sql: 'sql', sqlite: 'sql', ddl: 'sql', dml: 'sql', // 标准 SQL 语句
    mysql: 'mysql', // MySQL 方言
    postgres: 'pgsql', postgresql: 'pgsql', pgsql: 'pgsql', plpgsql: 'pgsql', // PostgreSQL 方言
    cypher: 'cypher', opencypher: 'cypher', // 图数据库 Cypher 查询
    sparql: 'sparql', rq: 'sparql', // SPARQL 语义查询
    redis: 'redis', // Redis 命令脚本
    coffee: 'coffeescript', coffeescript: 'coffeescript', // CoffeeScript 脚本
    dart: 'dart', // Dart 源码
    scala: 'scala', sc: 'scala', sbt: 'scala', // Scala 源码及构建定义
    ex: 'elixir', exs: 'elixir', elixir: 'elixir', // Elixir 源码与脚本
    clj: 'clojure', cljs: 'clojure', cljc: 'clojure', edn: 'clojure', clojure: 'clojure', // Clojure 方言集合
    objc: 'objective-c', 'obj-c': 'objective-c', 'objective-c': 'objective-c', objectivec: 'objective-c', m: 'objective-c', mm: 'objective-c', // Objective-C/C++ 源码
    sol: 'sol', solidity: 'sol', // Solidity 智能合约
    proto: 'proto', protobuf: 'proto', // Protocol Buffers 定义
    gql: 'graphql', graphql: 'graphql', // GraphQL 查询及 Schema
    hbs: 'handlebars', handlebars: 'handlebars', // Handlebars 模版
    pug: 'pug', jade: 'pug', // Pug HTML 模版
    razor: 'razor', cshtml: 'razor', // ASP.NET Razor 视图
    twig: 'twig', // Twig 模版引擎
    liquid: 'liquid', // Liquid 模版引擎
    vb: 'vb', vba: 'vb', vbscript: 'vb', visualbasic: 'vb', // Visual Basic 系列
    pas: 'pascal', pascal: 'pascal', // Pascal 源码
    jl: 'julia', julia: 'julia', // Julia 科学计算源码
    tcl: 'tcl', tcltk: 'tcl', // Tcl/Tk 脚本
    wgsl: 'wgsl', // WebGPU 着色器语言
    scm: 'scheme', ss: 'scheme', rkt: 'scheme', scheme: 'scheme', // Scheme 与 Racket 方言
    sv: 'systemverilog', svh: 'systemverilog', systemverilog: 'systemverilog', // SystemVerilog 硬件描述
    v: 'verilog', vh: 'verilog', verilog: 'verilog', // Verilog 硬件描述
    tsp: 'typespec', typespec: 'typespec', // TypeSpec API 描述语言
    qs: 'qsharp', qsharp: 'qsharp', // Q# 量子计算语言
    ftl: 'freemarker2', freemarker: 'freemarker2', freemarker2: 'freemarker2', // FreeMarker 模版
    aes: 'aes', sophia: 'aes', // Sophia 合约语言
    asm: 'mips', mips: 'mips', s: 'mips', // 汇编语言后缀
    st: 'st', iecst: 'st', // 结构化文本工控语言
    apex: 'apex', cls: 'apex', // Salesforce Apex 类文件
    abap: 'abap', // SAP ABAP 源码
}
const BASENAME_LANGS = { // 特殊完整文件名到语言标识映射
    dockerfile: 'dockerfile', // Docker 默认构建文件名
    containerfile: 'dockerfile', // Podman 构建文件名
    '.gitignore': 'ini', // Git 忽略规则文件
    '.gitattributes': 'ini', // Git 属性配置文件
    '.gitmodules': 'ini', // Git 子模块配置
    '.dockerignore': 'ini', // Docker 忽略规则文件
    '.npmignore': 'ini', // npm 发布忽略配置
    '.editorconfig': 'ini', // 编辑器统一风格配置
    '.gitconfig': 'ini', // Git 配置文件
    '.npmrc': 'ini', // npm 运行配置
    '.yarnrc': 'ini', // Yarn 运行配置
    '.env': 'ini', // 环境变量文件
    'cargo.lock': 'ini', // Rust 依赖锁文件
    'poetry.lock': 'ini', // Poetry 依赖锁文件
    'uv.lock': 'ini', // uv 依赖锁文件
    pipfile: 'ini', // Pipenv 清单文件
    '.eslintrc': 'json', // ESLint JSON 配置
    '.prettierrc': 'json', // Prettier 格式化配置
    '.babelrc': 'json', // Babel 转译配置
    '.jshintrc': 'json', // JSHint 检查配置
    '.jscsrc': 'json', // JSCS 风格配置
    '.swcrc': 'json', // SWC 编译配置
    'package.json': 'json', // 前端包清单
    'tsconfig.json': 'json', // TypeScript 工程配置
    'jsconfig.json': 'json', // JavaScript 工程配置
    'tauri.conf.json': 'json', // Tauri 应用配置
    gemfile: 'ruby', // Bundler 依赖文件
    rakefile: 'ruby', // Rake 任务脚本
    podfile: 'ruby', // CocoaPods 依赖文件
    vagrantfile: 'ruby', // Vagrant 虚拟机定义
    brewfile: 'ruby', // Homebrew 包清单
    fastfile: 'ruby', // Fastlane 自动化配置
    jakefile: 'javascript', // Jake 构建脚本
    'cmakelists.txt': 'plaintext', // CMake 构建配置清单
    makefile: 'shell', // Make 构建脚本
    gnumakefile: 'shell', // GNU Make 脚本
}
export const normalizeCodeLang = (value) => { // 将任意语言标记归一化为 Monaco 支持标识
    let raw = String(value || '').trim().toLowerCase() // 统一转小写并去除首尾空白
    if (!raw) return 'plaintext' // 空语言名回退纯文本
    raw = raw.replace(/^(?:language-|lang-)/, '') // 剥离 markdown 代码块常见前缀
    raw = raw.split(/[\s{,]/)[0] || '' // 截去附加高亮参数保留主语言名
    if (!raw) return 'plaintext' // 清洗后为空则回退纯文本
    if (Object.prototype.hasOwnProperty.call(LANG_ALIASES, raw)) return LANG_ALIASES[raw] // 命中别名表优先返回标准标识
    if (MONACO_LANGS.has(raw)) return raw // 直接属于 Monaco 内置语言则透传
    return 'plaintext' // 未收录语言回退纯文本
}
export const langForPath = (path) => { // 根据文件路径推断对应 Monaco 语法高亮语言
    const base = (String(path || '').split(/[\\/]/).pop() || '').trim().toLowerCase() // 提取小写文件名
    if (!base) return 'plaintext' // 空路径回退纯文本
    if (Object.prototype.hasOwnProperty.call(BASENAME_LANGS, base)) return BASENAME_LANGS[base] // 优先匹配完整特殊文件名
    if (base.startsWith('dockerfile.') || base.endsWith('.dockerfile') || base.startsWith('containerfile.')) { // 匹配容器构建变体命名
        return 'dockerfile' // 返回 Dockerfile 高亮
    }
    if (base.startsWith('.env.')) return 'ini' // 多环境 .env.* 文件按 ini 处理
    const dot = base.lastIndexOf('.') // 定位末位扩展名分隔符
    if (dot <= 0) return 'plaintext' // 无扩展名或隐藏点文件回退纯文本
    const ext = base.slice(dot + 1) // 截取后缀名部分
    if (!ext) return 'plaintext' // 空后缀回退纯文本
    if (Object.prototype.hasOwnProperty.call(LANG_ALIASES, ext)) return LANG_ALIASES[ext] // 查找后缀名映射别名
    if (MONACO_LANGS.has(ext)) return ext // 原生支持后缀直接返回
    return 'plaintext' // 未知后缀回退纯文本
}
