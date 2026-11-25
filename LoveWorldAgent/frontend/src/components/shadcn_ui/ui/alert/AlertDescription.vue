<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <div :class="cn('text-xs leading-5 text-muted-foreground', props.class)">
        <slot />
    </div>
</template>
