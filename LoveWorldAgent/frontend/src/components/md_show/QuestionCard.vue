<template>
    <div v-if="settled" class="settled" role="button" tabindex="0" :aria-expanded="expanded" @click="expanded = !expanded" @keydown.enter.prevent="expanded = !expanded">
        <CircleCheck :size="12" />
        <span class="settled-text">提问 · {{ answered ? '已回答' : '已处理' }}</span>
        <ChevronRight class="arrow" :class="{ open: expanded }" :size="12" />
    </div>
    <div v-if="!settled || expanded" class="question-card" role="group" aria-label="智能体提问">
        <div class="question-head">
            <CircleQuestionMark :size="14" class="question-icon" />
            <div class="question-titles">
                <span class="question-title">需要你的确认</span>
                <span class="question-sub">{{ answered ? '已回答，任务已继续' : '请回答以下问题，回答后将自动继续任务' }}</span>
            </div>
        </div>
        <div v-for="(item, index) in questions" :key="index" class="question-item">
            <div class="question-text"><span class="question-index">{{ index + 1 }}.</span>{{ item.question }}</div>
            <div v-if="answered" class="question-answer">A: {{ answerText(item, index) }}</div>
            <template v-else>
                <template v-if="item.options && item.options.length">
                    <div class="question-options">
                        <template v-if="item.mode === 'multiple'">
                            <span v-for="option in item.options" :key="option" class="option-row" @click="toggleChecked(index, option)">
                                <Checkbox :model-value="isChecked(index, option)" :disabled="locked" @click.stop @update:model-value="(checked) => { if (!locked) toggleOption(index, option, checked === true) }" />
                                <span>{{ option }}</span>
                            </span>
                        </template>
                        <RadioGroup v-else class="flex flex-col items-start gap-1" :model-value="draftOf(index)" :disabled="locked" @update:model-value="(value) => setDraft(index, value)">
                            <span v-for="option in item.options" :key="option" class="option-row" @click="selectOption(index, option)">
                                <RadioGroupItem :value="option" @click.stop />
                                <span>{{ option }}</span>
                            </span>
                        </RadioGroup>
                    </div>
                    <div v-if="selectedCustomOptions(item, index).length" class="custom-inputs">
                        <Input
                            v-for="option in selectedCustomOptions(item, index)"
                            :key="option"
                            :model-value="customInputOf(index, option)"
                            class="custom-input"
                            :disabled="locked"
                            :placeholder="customPlaceholder(item, index, option)"
                            @update:model-value="(value) => setCustomInput(index, option, value)"
                        />
                    </div>
                </template>
                <Textarea v-else class="question-textarea" :model-value="draftOf(index)" rows="2" :disabled="locked" placeholder="请输入你的回答" @update:model-value="(value) => setDraft(index, value)" />
                <Input v-if="item.customNote" v-model="notes[index]" class="note-input" :disabled="locked" placeholder="补充说明（可选）" />

            </template>
        </div>
        <div v-if="part.expired" class="question-foot">
            <span class="question-progress">审批已失效（超时或后端已重启），请在下方重新发送本轮需求</span>
        </div>
        <div v-else-if="!questions.length" class="question-answer">等待提问</div>
        <div v-else-if="!answered" class="question-foot">
            <span class="question-progress">已答 {{ answeredCount }} / {{ questions.length }}</span>
            <Button size="sm" :disabled="!allAnswered || part.decision === 'pending'" @click="submit">
                <Clock3 v-if="part.decision === 'pending'" :size="12" />
                全部回答后继续
            </Button>
        </div>
    </div>
</template>

<script setup>
import { computed, reactive, ref } from 'vue'
import { ChevronRight, CircleCheck, CircleQuestionMark, Clock3 } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import { Checkbox } from '@/components/shadcn_ui/ui/checkbox'
import { Input } from '@/components/shadcn_ui/ui/input'
import { RadioGroup, RadioGroupItem } from '@/components/shadcn_ui/ui/radio-group'
import { Textarea } from '@/components/shadcn_ui/ui/textarea'
const props = defineProps({ part: { type: Object, required: true } })
const emit = defineEmits(['answer'])
const expanded = ref(false)
const drafts = reactive({})
const notes = reactive({})
const customInputs = reactive({})
const questions = computed(() => (Array.isArray(props.part.questions) ? props.part.questions : []))
const answered = computed(() => Array.isArray(props.part.answers) && props.part.answers.length > 0)
const settled = computed(() => answered.value || Boolean(props.part.decision && props.part.decision !== 'pending'))
const draftKey = (index) => `${props.part.partKey || ''}:${index}`
const draftOf = (index) => {
    const value = drafts[draftKey(index)]
    const item = questions.value[index] || {}
    if (item.mode === 'multiple') return Array.isArray(value) ? value : []
    return value === undefined ? '' : value
}
const setDraft = (index, value) => { drafts[draftKey(index)] = value }
const locked = computed(() => Boolean(props.part.decision) || Boolean(props.part.expired))
const isCustomOption = (option) => String(option || '').includes('自定义')
const customKey = (index, option) => `${draftKey(index)}:${String(option || '')}`
const customInputOf = (index, option) => String(customInputs[customKey(index, option)] ?? '')
const setCustomInput = (index, option, value) => { customInputs[customKey(index, option)] = String(value ?? '') }
const customTextOf = (index, option) => customInputOf(index, option).trim()
const selectedCustomOptions = (item, index) => {
    if (!item || !Array.isArray(item.options) || !item.options.length) return []
    const value = draftOf(index)
    if (item.mode === 'multiple') return (Array.isArray(value) ? value : []).filter((option) => isCustomOption(option))
    const selected = String(value || '')
    return selected && isCustomOption(selected) ? [selected] : []
}
const customPlaceholder = (item, index, option) => (
    selectedCustomOptions(item, index).length > 1
        ? `请输入「${option}」内容（必填）`
        : '请输入自定义内容（必填）'
)
const isChecked = (index, option) => {
    const value = draftOf(index)
    return Array.isArray(value) && value.includes(option)
}
const toggleOption = (index, option, checked) => {
    const current = Array.isArray(draftOf(index)) ? draftOf(index) : []
    const next = checked ? [...new Set([...current, option])] : current.filter((item) => item !== option)
    setDraft(index, next)
}
const toggleChecked = (index, option) => {
    if (locked.value) return
    toggleOption(index, option, !isChecked(index, option))
}
const selectOption = (index, value) => {
    if (locked.value) return
    setDraft(index, value)
}
const isQuestionAnswered = (item, index) => {
    const value = draftOf(index)
    if (!item || !Array.isArray(item.options) || !item.options.length) return Boolean(String(value || '').trim())
    if (item.mode === 'multiple') {
        const list = Array.isArray(value) ? value : []
        return list.length > 0 && list.every((option) => !isCustomOption(option) || Boolean(customTextOf(index, option)))
    }
    const selected = String(value || '')
    return Boolean(selected.trim()) && (!isCustomOption(selected) || Boolean(customTextOf(index, selected)))
}
const answerOf = (item, index) => {
    const value = draftOf(index)
    if (!item || !Array.isArray(item.options) || !item.options.length) return String(value || '').trim()
    if (item.mode === 'multiple') {
        const list = Array.isArray(value) ? value : []
        return list.map((option) => (isCustomOption(option) ? customTextOf(index, option) : option)).join('、')
    }
    const selected = String(value || '')
    return isCustomOption(selected) ? customTextOf(index, selected) : selected.trim()
}
const answerText = (item, index) => {
    const list = Array.isArray(props.part.answers) ? props.part.answers : []
    const hit = list.find((entry) => String(entry.question || '') === String(item.question || '')) || list[index]
    return hit ? String(hit.answer || hit.text || '') : ''
}
const answeredCount = computed(() => questions.value.filter((item, index) => isQuestionAnswered(item, index)).length)
const allAnswered = computed(() => questions.value.length > 0 && questions.value.every((item, index) => isQuestionAnswered(item, index)))
const submit = () => {
    if (!allAnswered.value || props.part.decision || props.part.expired) return
    const list = questions.value.map((item, index) => ({
        question: String(item.question || ''),
        answer: answerOf(item, index),
        note: String(notes[index] || ''),
    }))
    emit('answer', props.part, list)
}
</script>

<style scoped>
.settled { display: flex; align-items: center; gap: 6px; padding: 4px 8px; border: 1px dashed var(--border); border-radius: 6px; color: var(--muted-foreground); font-size: 12px; cursor: pointer; user-select: none; }
.settled:focus-visible { outline: 1px solid var(--primary); outline-offset: -1px; }
.settled-text { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.settled .arrow { transition: transform 0.15s ease; }
.settled .arrow.open { transform: rotate(90deg); }
.question-card { width: 100%; display: flex; flex-direction: column; gap: 8px; padding: 8px 10px; border: 1px solid color-mix(in srgb, var(--primary) 35%, transparent); border-radius: 6px; background: color-mix(in srgb, var(--primary) 12%, transparent); font-size: 12px; }
.question-head { display: flex; align-items: flex-start; gap: 6px; }
.question-icon { flex: none; margin-top: 1px; color: var(--primary); }
.question-titles { display: flex; flex-direction: column; gap: 2px; }
.question-title { font-weight: 600; color: var(--foreground); }
.question-sub { color: var(--muted-foreground); line-height: 1.6; }
.question-item { display: flex; flex-direction: column; gap: 6px; padding: 6px 8px; border-radius: 6px; background: var(--card); }
.question-text { display: flex; gap: 4px; line-height: 1.6; color: var(--foreground); }
.question-index { flex: none; color: var(--primary); font-weight: 600; }
.question-answer { color: var(--muted-foreground); line-height: 1.6; word-break: break-word; }
.question-options { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
.option-row { display: inline-flex; align-items: center; gap: 6px; min-height: 22px; margin-right: 0; font-size: 12px; color: var(--foreground); cursor: pointer; }
.question-textarea { min-height: 0; resize: none; font-size: 12px; }
.custom-inputs { display: flex; flex-direction: column; gap: 4px; }
.custom-input { height: 24px; font-size: 12px; }
.note-input { height: 24px; font-size: 12px; }

.question-foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.question-progress { color: var(--muted-foreground); }
</style>