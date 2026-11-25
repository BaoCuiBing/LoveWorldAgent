<script setup>
import { computed } from 'vue'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: String, default: '' },
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['update:modelValue'])
const model = computed({
    get: () => props.modelValue,
    set: (value) => emits('update:modelValue', value),
})
</script>

<template>
    <textarea
        v-model="model"
        :class="cn('flex min-h-16 w-full resize-y rounded-md border border-input bg-transparent px-2.5 py-1.5 text-xs leading-5 text-foreground shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50', props.class)"
    />
</template>
