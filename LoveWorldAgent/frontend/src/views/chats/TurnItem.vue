<template>
    <div class="turn" :data-turn-key="turn.turnKey">
        <SelfMessageSee v-if="userMessage" :message="userMessage" @recall="$emit('recall', turn)" @open-file="(payload) => $emit('open-file', payload)" />
        <div v-if="rollback" class="rollback-bar">
            <Info class="rollback-icon" :size="16" />
            <div class="rollback-text">
                <div>确定要回滚到此步骤并重新开始吗？将删除本地与服务端此轮及之后的对话。</div>
                <div class="rollback-sub">此操作不可撤销。</div>
            </div>
            <div class="rollback-actions">
                <Button variant="outline" size="sm" @click="$emit('rollback-cancel')">取消</Button>
                <Button size="sm" @click="$emit('rollback-confirm', turn)">确认</Button>
            </div>
        </div>
        <AiMessageSee :assistant="turn.assistant" :turn-key="turn.turnKey" :is-last-turn="isLast" @decision="(part, decision) => $emit('decision', turn, part, decision)" @answer="(part, answers) => $emit('answer', turn, part, answers)" @retry="$emit('retry', turn)" @delete="$emit('delete', turn)" @open-file="(payload) => $emit('open-file', payload)" @continue="$emit('continue', turn)" />
    </div>
</template>

<script setup>
import { computed } from 'vue'
import { Info } from 'lucide-vue-next'
import { Button } from '@/components/shadcn_ui/ui/button'
import SelfMessageSee from './SelfMessageSee.vue'
import AiMessageSee from './AiMessageSee.vue'
const props = defineProps({ turn: { type: Object, required: true }, rollback: { type: Boolean, default: false }, isLast: { type: Boolean, default: false } })
defineEmits(['recall', 'delete', 'retry', 'rollback-confirm', 'rollback-cancel', 'decision', 'answer', 'open-file', 'continue'])
const userMessage = computed(() => (props.turn.user ? { ...props.turn.user, id: props.turn.turnKey, role: 'user' } : null))
</script>

<style scoped>
.turn { display: flex; flex-direction: column; gap: 10px; width: 100%; min-width: 0; }
.rollback-bar { display: flex; align-items: flex-start; gap: 8px; padding: 10px 12px; border: 1px solid color-mix(in srgb, var(--primary) 35%, transparent); border-radius: 6px; background: color-mix(in srgb, var(--primary) 12%, transparent); }
.rollback-icon { flex: none; margin-top: 1px; color: var(--primary); }
.rollback-text { flex: 1; min-width: 0; font-size: 12px; line-height: 1.6; color: var(--foreground); }
.rollback-sub { color: var(--muted-foreground); }
.rollback-actions { flex: none; display: flex; align-items: center; gap: 6px; }
</style>