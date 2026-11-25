<script setup>
import { ScrollAreaCorner, ScrollAreaRoot, ScrollAreaViewport } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
import ScrollBar from './ScrollBar.vue'
const props = defineProps({
    type: { type: String, default: 'hover' },
    scrollHideDelay: { type: Number, default: 600 },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <ScrollAreaRoot :type="props.type" :scroll-hide-delay="props.scrollHideDelay" :class="cn('relative overflow-hidden', props.class)">
        <ScrollAreaViewport class="size-full rounded-[inherit]">
            <slot />
        </ScrollAreaViewport>
        <ScrollBar />
        <ScrollAreaCorner />
    </ScrollAreaRoot>
</template>
