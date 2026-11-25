<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <span :class="cn('ml-auto text-[11px] tracking-widest text-muted-foreground', props.class)">
        <slot />
    </span>
</template>
