<script setup>
import { DropdownMenuItem } from 'reka-ui'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    disabled: Boolean,
    textValue: { type: String, default: undefined },
    class: { type: [String, Array, Object], default: '' },
})
const emits = defineEmits(['select'])
</script>

<template>
    <DropdownMenuItem
        :disabled="props.disabled"
        :text-value="props.textValue"
        :class="cn('relative flex cursor-default select-none items-center gap-2 rounded-sm px-2 py-1.5 text-xs outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:size-3.5 [&_svg]:shrink-0', props.class)"
        @select="emits('select', $event)"
    >
        <slot />
    </DropdownMenuItem>
</template>
