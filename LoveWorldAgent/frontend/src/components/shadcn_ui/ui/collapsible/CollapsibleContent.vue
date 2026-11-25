<script setup>
import { CollapsibleContent } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    forceMount: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <CollapsibleContent :force-mount="props.forceMount" :class="cn('overflow-hidden', props.class)">
        <slot />
    </CollapsibleContent>
</template>
