<script setup>
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    as: { type: [String, Object], default: 'p' },
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <component :is="props.as" :class="cn('text-xs text-muted-foreground', props.class)">
        <slot />
    </component>
</template>
