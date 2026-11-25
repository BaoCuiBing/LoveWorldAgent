// js_builder 实现：优化版快照脚本（无 3s 空转、选择器前置过滤、交互元素优先、双层 ref 映射）、ref 解析与完整按键表
#include "utils/js_builder.h" // 自身声明
#include <windows.h> // VkKeyScanW
#include <cctype> // toupper/tolower
#include <cstdio> // snprintf
#include <cstring> // strcmp
#include "include/internal/cef_types.h" // EVENTFLAG_*
namespace js {
namespace {
const char* kSnapshotJs = R"JS((function(){
function build(){
var INTERACTIVE='a[href],button,input:not([type="hidden"]),select,textarea,summary,[onclick],[role="button"],[role="link"],[role="textbox"],[role="checkbox"],[role="radio"],[role="combobox"],[role="menuitem"],[role="tab"],[role="switch"],[role="option"],[tabindex]:not([tabindex="-1"]),[contenteditable=""],[contenteditable="true"],[contenteditable="plaintext-only"]';
var TEXTUAL='h1,h2,h3,h4,h5,h6,p,li,td,th,label,img,form,nav,main,header,footer,section,article';
var MAX_INTERACTIVE=320;var MAX_TEXT=160;var LIMIT=420;var seq=0;
function clean(s){return (s==null?'':String(s)).replace(/\s+/g,' ').trim();}
function short(s,n){return s.length>n?(s.slice(0,n)+'...'):s;}
function roleOf(el){
var r=clean(el.getAttribute('role'));if(r){return r;}
var t=el.tagName.toLowerCase();var ty=clean(el.getAttribute('type')).toLowerCase();
if(t==='a'&&el.hasAttribute('href')){return 'link';}
if(t==='button'){return 'button';}
if(t==='input'){return ty==='checkbox'?'checkbox':(ty==='radio'?'radio':((ty==='submit'||ty==='button')?'button':'textbox'));}
if(t==='select'){return 'combobox';}
if(t==='textarea'){return 'textbox';}
if(t==='img'){return 'img';}
if(t==='li'){return 'listitem';}
if(/^h[1-6]$/.test(t)){return 'heading';}
if(t==='nav'){return 'navigation';}
if(t==='form'){return 'form';}
return t;}
function nameOf(el){
var t=el.tagName.toLowerCase();var ty=clean(el.getAttribute('type')).toLowerCase();
var n=clean(el.getAttribute('aria-label'));if(n){return short(n,80);}
if(t==='input'||t==='textarea'){
if(!(t==='input'&&ty==='password')){n=clean(el.value);if(n){return short(n,60);}}
n=clean(el.getAttribute('placeholder'));if(n){return short(n,60);}}
n=clean(el.textContent);if(n){return short(n,80);}
n=clean(el.getAttribute('alt'))||clean(el.getAttribute('title'))||clean(el.getAttribute('name'));return short(n,60);}
function isVisible(el){
if(el.hasAttribute('hidden')||el.getAttribute('aria-hidden')==='true'){return false;}
if(typeof el.checkVisibility==='function'){
return el.checkVisibility({checkOpacity:false,checkVisibilityCSS:true});
}
var st=window.getComputedStyle(el);
return !(!st||st.display==='none'||st.visibility==='hidden');}
var prev=document.body?document.body.querySelectorAll('[data-lwa-ref]'):[];
for(var i=0;i<prev.length;i++){prev[i].removeAttribute('data-lwa-ref');}
window.__lwaRefMap=Object.create(null);
var nodes=document.body?document.body.querySelectorAll(INTERACTIVE+','+TEXTUAL):[];
var interactiveLines=[];var textLines=[];var allLines=[];
for(var j=0;j<nodes.length;j++){
if(interactiveLines.length>=MAX_INTERACTIVE&&textLines.length>=MAX_TEXT){break;}
var el=nodes[j];
var tag=el.tagName.toLowerCase();
if(tag==='script'||tag==='style'||tag==='noscript'){continue;}
var interactive=el.matches(INTERACTIVE);
if(interactive&&interactiveLines.length>=MAX_INTERACTIVE){continue;}
if(!interactive&&textLines.length>=MAX_TEXT){continue;}
if(!isVisible(el)){continue;}
var name=nameOf(el);
if(!interactive&&!name){continue;}
var ref='';var extra='';var role=roleOf(el);var ty=clean(el.getAttribute('type')).toLowerCase();
if(interactive){
ref='e'+(++seq);
el.setAttribute('data-lwa-ref',ref);
window.__lwaRefMap[ref]={el:el,tag:tag,role:role,name:name,type:ty,id:el.id||'',aria:clean(el.getAttribute('aria-label'))};
if(tag==='input'||tag==='textarea'){
if(tag==='input'&&ty==='password'){extra=' value="'+(el.value?'***':'')+'"';}
else{extra=' value="'+short(clean(el.value),40)+'"';}
}else if(tag==='a'&&el.href){extra=' url="'+short(el.href,90)+'"';}
}
var line='- '+role+(name?(' "'+name+'"'):'')+extra+(ref?(' [ref='+ref+']'):'');
if(interactive){interactiveLines.push(line);}else{textLines.push(line);}
allLines.push(line);
}
var truncated=(interactiveLines.length+textLines.length)>LIMIT;
var lines;
if(!truncated){lines=allLines;}
else{
var rem=LIMIT-interactiveLines.length;
lines=rem>0?interactiveLines.concat(textLines.slice(0,rem)):interactiveLines.slice(0,LIMIT);
}
return 'URL: '+location.href+'\nTITLE: '+document.title+'\nREFS: '+seq+(truncated?'\n(部分非交互文本已省略，交互元素已优先保留)':'')+'\n'+lines.join('\n');
}
if(document.readyState!=='loading'&&document.body){return Promise.resolve(build());}
return new Promise(function(resolve){
var end=Date.now()+400;
function tick(){if((document.readyState!=='loading'&&document.body)||Date.now()>=end){resolve(build());return;}setTimeout(tick,50);}
tick();
});
})())JS";

const char* kReadTextJs = R"JS((function(){
var t=document.body?(document.body.innerText||''):'';
t=t.replace(/[ \t]+/g,' ');
return t.length>20000?(t.slice(0,20000)+'\n...(已截断)'):t;
})())JS";

struct KeyDef { const char* name; const char* code; int vk; char16_t ch; bool printable; };
const KeyDef kKeys[] = {
    {"Enter", "Enter", 13, u'\r', true},
    {"Escape", "Escape", 27, 0, false},
    {"Tab", "Tab", 9, u'\t', true},
    {"Backspace", "Backspace", 8, 0, false},
    {"Delete", "Delete", 46, 0, false},
    {"Insert", "Insert", 45, 0, false},
    {"ArrowUp", "ArrowUp", 38, 0, false},
    {"ArrowDown", "ArrowDown", 40, 0, false},
    {"ArrowLeft", "ArrowLeft", 37, 0, false},
    {"ArrowRight", "ArrowRight", 39, 0, false},
    {"Home", "Home", 36, 0, false},
    {"End", "End", 35, 0, false},
    {"PageUp", "PageUp", 33, 0, false},
    {"PageDown", "PageDown", 34, 0, false},
    {"Space", "Space", 32, u' ', true},
    {"F1", "F1", VK_F1, 0, false},
    {"F2", "F2", VK_F2, 0, false},
    {"F3", "F3", VK_F3, 0, false},
    {"F4", "F4", VK_F4, 0, false},
    {"F5", "F5", VK_F5, 0, false},
    {"F6", "F6", VK_F6, 0, false},
    {"F7", "F7", VK_F7, 0, false},
    {"F8", "F8", VK_F8, 0, false},
    {"F9", "F9", VK_F9, 0, false},
    {"F10", "F10", VK_F10, 0, false},
    {"F11", "F11", VK_F11, 0, false},
    {"F12", "F12", VK_F12, 0, false}
};

std::string LowerAscii(std::string s) {
    for (char& ch : s) ch = static_cast<char>(std::tolower(static_cast<unsigned char>(ch)));
    return s;
}

bool TryModifierToken(const std::string& token, uint32_t& mods) {
    const std::string low = LowerAscii(token);
    if (low == "ctrl" || low == "control") { mods |= EVENTFLAG_CONTROL_DOWN; return true; }
    if (low == "alt") { mods |= EVENTFLAG_ALT_DOWN; return true; }
    if (low == "shift") { mods |= EVENTFLAG_SHIFT_DOWN; return true; }
    if (low == "meta" || low == "command" || low == "cmd") { mods |= EVENTFLAG_COMMAND_DOWN; return true; }
    return false;
}

std::string PunctCode(char ch) {
    switch (ch) {
        case '-': case '_': return "Minus";
        case '=': case '+': return "Equal";
        case '[': case '{': return "BracketLeft";
        case ']': case '}': return "BracketRight";
        case '\\': case '|': return "Backslash";
        case ';': case ':': return "Semicolon";
        case '\'': case '"': return "Quote";
        case ',': case '<': return "Comma";
        case '.': case '>': return "Period";
        case '/': case '?': return "Slash";
        case '`': case '~': return "Backquote";
        default: return std::string();
    }
}
}  // namespace

const char* kSnapshot = kSnapshotJs;
const char* kReadText = kReadTextJs;

std::string SafeRef(const std::string& ref) {
    std::string out;
    for (const char ch : ref) {
        if ((ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || ch == '_' || ch == '-') out += ch;
    }
    return out;
}

std::string RefSelector(const std::string& ref) {
    return std::string("[data-lwa-ref=\"") + SafeRef(ref) + "\"]";
}

std::string ResolveRefExpr(const std::string& ref) {
    const std::string safe = SafeRef(ref);
    return "(function(ref){"
           "var m=window.__lwaRefMap&&window.__lwaRefMap[ref];"
           "if(m&&m.el&&m.el.isConnected){return m.el;}"
           "var d=document.querySelector('[data-lwa-ref=\"'+ref+'\"]');"
           "if(d){return d;}"
           "if(m){"
           "if(m.id){var byId=document.getElementById(m.id);if(byId){return byId;}}"
           "if(m.tag){"
           "var cand=document.querySelectorAll(m.tag);var hits=[];"
           "function clean(s){return (s==null?'':String(s)).replace(/\\s+/g,' ').trim();}"
           "function short(s,n){return s.length>n?(s.slice(0,n)+'...'):s;}"
           "function nameOf(c){"
           "var ct=c.tagName.toLowerCase();var cty=clean(c.getAttribute('type')).toLowerCase();"
           "var n=clean(c.getAttribute('aria-label'));if(n){return short(n,80);}"
           "if(ct==='input'||ct==='textarea'){"
           "if(!(ct==='input'&&cty==='password')){n=clean(c.value);if(n){return short(n,60);}}"
           "n=clean(c.getAttribute('placeholder'));if(n){return short(n,60);}}"
           "n=clean(c.textContent);if(n){return short(n,80);}"
           "n=clean(c.getAttribute('alt'))||clean(c.getAttribute('title'))||clean(c.getAttribute('name'));return short(n,60);"
           "}"
           "for(var i=0;i<cand.length;i++){"
           "var c=cand[i];"
           "var cn=nameOf(c);"
           "var cr=clean(c.getAttribute('role'));"
           "if((!m.name||cn===m.name)&&(!m.role||!cr||cr===m.role)){hits.push(c);}"
           "}"
           "if(hits.length===1){return hits[0];}"
           "}"
           "}"
           "throw new Error('ref 已失效，请重新 browser_snapshot');"
           "})(" + Literal(safe) + ")";
}

std::string Element(const std::string& ref, const std::string& tail) {
    return "(function(){var el=" + ResolveRefExpr(ref) + ";" + tail + "})()";
}

std::string Literal(const std::string& utf8) {
    std::string out = "\"";
    for (const char ch : utf8) {
        switch (ch) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default:
                if (static_cast<unsigned char>(ch) < 0x20) {
                    char buffer[8] = {0};
                    std::snprintf(buffer, sizeof(buffer), "\\u%04x", static_cast<unsigned char>(ch));
                    out += buffer;
                } else {
                    out += ch;
                }
        }
    }
    out += "\"";
    return out;
}

int MouseButtonCode(const std::string& raw) {
    if (raw == "right") return 2;
    if (raw == "middle") return 1;
    return 0;
}

bool QueryKey(const std::string& raw, KeyInfo& out) {
    out = KeyInfo{};
    if (raw.empty()) return false;
    std::string key = raw;
    uint32_t mods = 0;
    while (true) {
        const size_t plus = key.find('+');
        if (plus == std::string::npos || plus == 0 || plus + 1 >= key.size()) break;
        const std::string prefix = key.substr(0, plus);
        if (!TryModifierToken(prefix, mods)) break;
        key = key.substr(plus + 1);
    }
    out.modifiers = mods;
    const std::string low = LowerAscii(key);
    for (const KeyDef& def : kKeys) {
        if (key == def.name || low == LowerAscii(def.name) || (key == " " && std::strcmp(def.name, "Space") == 0)) {
            out.name = def.name;
            out.code = def.code;
            out.vk = def.vk;
            out.ch = def.ch;
            out.printable = def.printable;
            return true;
        }
    }
    if (key.size() == 1) {
        const unsigned char uch = static_cast<unsigned char>(key[0]);
        if (uch >= 'a' && uch <= 'z') {
            const char upper = static_cast<char>(std::toupper(uch));
            out.name = key;
            out.code = std::string("Key") + upper;
            out.vk = static_cast<int>(upper);
            out.ch = static_cast<char16_t>(uch);
            out.printable = true;
            return true;
        }
        if (uch >= 'A' && uch <= 'Z') {
            out.name = key;
            out.code = std::string("Key") + static_cast<char>(uch);
            out.vk = static_cast<int>(uch);
            out.ch = static_cast<char16_t>(uch);
            out.printable = true;
            return true;
        }
        if (uch >= '0' && uch <= '9') {
            out.name = key;
            out.code = std::string("Digit") + static_cast<char>(uch);
            out.vk = static_cast<int>(uch);
            out.ch = static_cast<char16_t>(uch);
            out.printable = true;
            return true;
        }
        if (uch >= 0x20 && uch <= 0x7E) {
            const SHORT scan = ::VkKeyScanW(static_cast<WCHAR>(uch));
            if (scan != -1) {
                out.name = key;
                const std::string pcode = PunctCode(static_cast<char>(uch));
                out.code = pcode.empty() ? key : pcode;
                out.vk = scan & 0xFF;
                out.ch = static_cast<char16_t>(uch);
                out.printable = true;
                return true;
            }
        }
    }
    return false;
}
}  // namespace js
