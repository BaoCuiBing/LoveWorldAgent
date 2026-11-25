<script setup>
import { Separator } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    orientation: { type: String, default: 'horizontal' },
    decorative: { type: Boolean, default: true },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <Separator
        :orientation="props.orientation"
        :decorative="props.decorative"
        :class="cn('shrink-0 bg-border', props.orientation === 'vertical' ? 'h-full w-px' : 'h-px w-full', props.class)"
    />
</template>
