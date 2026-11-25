<script setup>
import { computed } from 'vue'
import { ProgressIndicator, ProgressRoot } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    modelValue: { type: Number, default: 0 },
    max: { type: Number, default: 100 },
    class: { type: [String, Array, Object], default: '' },
})
const percent = computed(() => {
    if (!props.max) return 0
    return Math.min(100, Math.max(0, (props.modelValue / props.max) * 100))
})
</script>

<template>
    <ProgressRoot
        :model-value="props.modelValue"
        :max="props.max"
        :class="cn('relative h-1.5 w-full overflow-hidden rounded-full bg-muted', props.class)"
    >
        <ProgressIndicator
            class="size-full flex-1 rounded-full bg-primary transition-transform duration-300"
            :style="{ transform: `translateX(-${100 - percent}%)` }"
        />
    </ProgressRoot>
</template>
