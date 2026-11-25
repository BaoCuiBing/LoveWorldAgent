<script setup>
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: String, default: '' },
    disabled: Boolean,
    group: { type: String, default: '' },
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['select'])
const command = inject('command', null)
const el = ref(null)
let id = 0
const keyword = computed(() => props.value || (el.value ? el.value.textContent : ''))
const visible = computed(() => (command ? command.isMatch(keyword.value) : true))
onMounted(() => {
    if (!command) return
    id = command.register(() => visible.value, props.group)
})
onBeforeUnmount(() => {
    if (command) command.unregister(id)
})
const onSelect = () => {
    if (props.disabled) return
    emits('select')
}
</script>

<template>
    <div
        v-show="visible"
        ref="el"
        :class="
            cn(
                'relative flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-xs outline-none transition-colors hover:bg-accent hover:text-accent-foreground',
                props.disabled && 'pointer-events-none opacity-50',
                props.class,
            )
        "
        @click="onSelect"
    >
        <slot />
    </div>
</template>
