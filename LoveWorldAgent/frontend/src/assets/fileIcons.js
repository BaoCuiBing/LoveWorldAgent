// 文件类型图标映射：图标源文件在 assets/file_icons（随包分发），文件树、编辑器标签与右栏上下文共用
import iconFolder from './file_icons/folder.svg'
import iconFile from './file_icons/file.svg'
import iconCode from './file_icons/code.svg'
import iconPy from './file_icons/py.svg'
import iconMd from './file_icons/md.svg'
import iconHtml from './file_icons/html.svg'
import iconJs from './file_icons/js.svg'
import iconTs from './file_icons/ts.svg'
import iconVue from './file_icons/vue.svg'
import iconCss from './file_icons/css.svg'
import iconScss from './file_icons/scss.svg'
import iconJson from './file_icons/json.svg'
import iconYaml from './file_icons/yaml.svg'
import iconSql from './file_icons/sql.svg'
import iconSh from './file_icons/sh.svg'
import iconPs1 from './file_icons/ps1.svg'
import iconTxt from './file_icons/txt.svg'
import iconImage from './file_icons/image.svg'
import iconPdf from './file_icons/pdf.svg'
import iconZip from './file_icons/zip.svg'
import iconCsv from './file_icons/csv.svg'
import iconXml from './file_icons/xml.svg'
import iconRs from './file_icons/rs.svg'
import iconCpp from './file_icons/cpp.svg'
import iconC from './file_icons/c.svg'
import iconH from './file_icons/h.svg'
import iconToml from './file_icons/toml.svg'
import iconBat from './file_icons/bat.svg'
import iconEnv from './file_icons/env.svg'
import iconGo from './file_icons/go.svg'
import iconJava from './file_icons/java.svg'
import iconPhp from './file_icons/php.svg'
import iconRb from './file_icons/rb.svg'
import iconKt from './file_icons/kt.svg'
import iconSwift from './file_icons/swift.svg'
import iconDart from './file_icons/dart.svg'
import iconLua from './file_icons/lua.svg'
export const ICON_FOLDER = iconFolder
export const ICON_FILE = iconFile
export const ICON_CODE = iconCode
const ICON_MAP = {
    py: iconPy, pyw: iconPy, md: iconMd, markdown: iconMd, html: iconHtml, htm: iconHtml, js: iconJs, mjs: iconJs,
    cjs: iconJs, jsx: iconJs, ts: iconTs, tsx: iconTs, vue: iconVue, css: iconCss, less: iconCss, scss: iconScss,
    sass: iconScss, json: iconJson, jsonc: iconJson, yaml: iconYaml, yml: iconYaml, sql: iconSql, sh: iconSh,
    bash: iconSh, zsh: iconSh, ps1: iconPs1, txt: iconTxt, log: iconTxt, png: iconImage, jpg: iconImage,
    jpeg: iconImage, gif: iconImage, webp: iconImage, bmp: iconImage, ico: iconImage, svg: iconImage,
    pdf: iconPdf, zip: iconZip, gz: iconZip, tar: iconZip, rar: iconZip, csv: iconCsv, tsv: iconCsv, xml: iconXml,
    doc: iconPdf, docx: iconPdf, xls: iconCsv, xlsx: iconCsv, ppt: iconFile, pptx: iconFile,
    rs: iconRs, c: iconC, cpp: iconCpp, cc: iconCpp, cxx: iconCpp, h: iconH, hpp: iconH, hxx: iconH,
    toml: iconToml, lock: iconToml, bat: iconBat, cmd: iconBat, env: iconEnv, ini: iconEnv, conf: iconEnv, cfg: iconEnv,
    go: iconGo, java: iconJava, php: iconPhp, rb: iconRb, kt: iconKt, kts: iconKt, swift: iconSwift, dart: iconDart, lua: iconLua,
}
const TONE_MAP = {
    py: 'tone-blue', css: 'tone-blue', less: 'tone-blue', scss: 'tone-blue', sass: 'tone-blue',
    cpp: 'tone-blue', cc: 'tone-blue', cxx: 'tone-blue', go: 'tone-blue', dart: 'tone-blue', lua: 'tone-blue',
    md: 'tone-purple', markdown: 'tone-purple', yaml: 'tone-purple', yml: 'tone-purple',
    h: 'tone-purple', hpp: 'tone-purple', hxx: 'tone-purple', php: 'tone-purple', kt: 'tone-purple', kts: 'tone-purple',
    js: 'tone-yellow', mjs: 'tone-yellow', cjs: 'tone-yellow', jsx: 'tone-yellow', ts: 'tone-yellow',
    tsx: 'tone-yellow', json: 'tone-yellow', jsonc: 'tone-yellow', env: 'tone-yellow', ini: 'tone-yellow',
    html: 'tone-orange', htm: 'tone-orange', xml: 'tone-orange', rs: 'tone-orange', toml: 'tone-orange',
    java: 'tone-orange', rb: 'tone-orange', swift: 'tone-orange',
    vue: 'tone-green', png: 'tone-green', jpg: 'tone-green', jpeg: 'tone-green',
    gif: 'tone-green', webp: 'tone-green', bmp: 'tone-green', ico: 'tone-green', svg: 'tone-green',
}
export const extOf = (name) => {
    const text = String(name || '')
    const index = text.lastIndexOf('.')
    return index === -1 ? '' : text.slice(index + 1).toLowerCase()
}
// 取图标：入参可以是文件名或项目相对路径；type 为 dir 时返回文件夹图标
export const fileIconOf = (nameOrPath, type = 'file') => {
    if (String(type || '').toLowerCase() === 'dir') return iconFolder
    const base = String(nameOrPath || '').split(/[\\/]/).pop()
    return ICON_MAP[extOf(base)] || iconFile
}
// 取配色类名（与文件树同名 class，供标签/上下文行复用）
export const toneOf = (nameOrPath, type = 'file') => {
    if (String(type || '').toLowerCase() === 'dir') return ''
    const base = String(nameOrPath || '').split(/[\\/]/).pop()
    return TONE_MAP[extOf(base)] || ''
}
