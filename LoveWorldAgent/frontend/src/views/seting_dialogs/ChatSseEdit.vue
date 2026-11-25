<template>
    <div class="chat-sse-edit">
        <div class="sse-section">
            <div class="sse-title">智能体主动提问</div>
            <div class="panel-card sse-row-card">
                <div class="sse-main">
                    <div class="sse-name">智能体主动提问</div>
                    <div class="sse-desc">允许智能体在任务不明确时主动向用户提问，澄清需求细节后指导执行。</div>
                </div>
                <Switch v-model="form.ask" @update:model-value="scheduleSave" />
            </div>
        </div>
        <div class="sse-section">
            <div class="sse-title">待办清单</div>
            <div class="panel-card sse-row-card">
                <div class="sse-main">
                    <div class="sse-name">待办清单</div>
                    <div class="sse-desc">允许智能体使用待办清单来跟踪任务进度</div>
                </div>
                <Switch v-model="form.todo" @update:model-value="scheduleSave" />
            </div>
        </div>
        <div class="sse-section">
            <div class="sse-title">任务状态通知</div>
            <div class="panel-card notify-card">
                <div class="notify-row">
                    <div class="sse-main">
                        <div class="sse-name">允许在任务完成或失败时接收通知</div>
                        <div class="sse-desc">允许在任务完成或失败时接收通知 请在 Windows 的系统设置 &gt; 通知中开启通知，以便及时收到提醒</div>
                    </div>
                    <div class="sse-switches">
                        <div class="sse-switch"><span>横幅</span><Switch v-model="form.notify.banner" @update:model-value="scheduleSave" /></div>
                        <div class="sse-switch"><span>声音</span><Switch v-model="form.notify.sound" @update:model-value="scheduleSave" /></div>
                    </div>
                </div>
                <div class="notify-row">
                    <span class="notify-label">音量设置</span>
                    <div class="volume-input">
                        <Input :model-value="form.notify.volume" type="number" min="0" max="100" class="volume-field" @change="onVolumeChange($event.target.value)" />
                        <span class="volume-suffix">%</span>
                    </div>
                </div>
                <div class="sound-head">提示音</div>
                <div class="sound-list">
                    <div v-for="item in sounds" :key="item.key" class="sound-row">
                        <Headphones class="sound-icon" :size="20" />
                        <div class="sound-main">
                            <span class="sound-name">{{ item.label }}</span>
                            <span class="sound-default">默认</span>
                        </div>
                        <Button variant="ghost" size="icon-sm" aria-label="播放提示音" @click="playSound(item.key)"><Play :size="18" /></Button>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<script setup>
import { onMounted, reactive, ref } from 'vue'
import { Headphones, Play } from 'lucide-vue-next'
import { Button } from '../../components/shadcn_ui/ui/button'
import { Input } from '../../components/shadcn_ui/ui/input'
import { Switch } from '../../components/shadcn_ui/ui/switch'
import { toast } from '../../components/shadcn_ui/lib/useToast'
import { chatSoundUrl, getChatSse, saveChatSse } from '../../api/request_data'
const sounds = [
    { key: 'win', label: '任务完成' },
    { key: 'wait', label: '等待操作' },
    { key: 'error', label: '异常打断' },
]
const form = reactive({
    ask: true,
    todo: true,
    notify: { banner: true, sound: true, volume: 100 },
})
const lastSaved = ref('')
const clone = () => JSON.stringify(form)
let timer = null
const scheduleSave = () => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(save, 500)
}
const save = async () => {
    try {
        const data = await saveChatSse(JSON.parse(clone()))
        form.ask = data.ask
        form.todo = data.todo
        Object.assign(form.notify, data.notify)
        lastSaved.value = clone()
    } catch (err) {
        if (lastSaved.value) {
            const previous = JSON.parse(lastSaved.value)
            form.ask = previous.ask
            form.todo = previous.todo
            Object.assign(form.notify, previous.notify)
        }
        toast.error(`保存对话设置失败：${err.message}`)
    }
}
const onVolumeChange = (value) => {
    const volume = Math.min(Math.max(Number(value) || 0, 0), 100)
    form.notify.volume = volume
    scheduleSave()
}
const playSound = (key) => {
    const audio = new Audio(chatSoundUrl(key))
    audio.volume = Math.min(Math.max(Number(form.notify.volume) / 100, 0), 1)
    audio.play().catch((err) => toast.error(`播放提示音失败：${err.message}`))
}
const loadConfig = async () => {
    try {
        const data = await getChatSse()
        form.ask = data.ask
        form.todo = data.todo
        Object.assign(form.notify, data.notify)
        lastSaved.value = clone()
    } catch (err) {
        toast.error(`读取对话设置失败：${err.message}`)
    }
}
onMounted(loadConfig)
</script>

<style scoped>
.chat-sse-edit { display: flex; flex-direction: column; gap: 18px; }
.sse-section { display: flex; flex-direction: column; gap: 8px; }
.sse-title { font-size: 14px; font-weight: 600; }
.sse-row-card { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 16px; }
.sse-main { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.sse-name { font-size: 13px; font-weight: 600; }
.sse-desc { font-size: 12px; color: var(--muted-foreground); }
.sse-switches { display: flex; flex: none; flex-direction: column; gap: 10px; padding-left: 16px; border-left: 1px solid var(--border); }
.sse-switch { display: flex; align-items: center; justify-content: space-between; gap: 14px; min-width: 108px; font-size: 12px; }
.notify-card { display: flex; flex-direction: column; padding: 0; }
.notify-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 14px 16px; }
.notify-row + .notify-row { border-top: 1px solid var(--border); }
.notify-label { font-size: 13px; }
.volume-input { position: relative; width: 180px; }
.volume-field { padding-right: 22px; }
.volume-suffix { position: absolute; right: 8px; top: 50%; transform: translateY(-50%); color: var(--muted-foreground); font-size: 11px; pointer-events: none; }
.sound-head { padding: 12px 16px 6px; font-size: 13px; font-weight: 600; border-top: 1px solid var(--border); }
.sound-list { display: flex; flex-direction: column; }
.sound-row { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
.sound-row:not(:last-child) { border-bottom: 1px solid var(--border); }
.sound-icon { box-sizing: content-box; padding: 8px; border-radius: 8px; color: var(--primary); background: color-mix(in srgb, var(--primary) 12%, transparent); }
.sound-main { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.sound-name { font-size: 13px; }
.sound-default { font-size: 11px; color: var(--muted-foreground); }
</style>
