<script setup>
import { SelectItem, SelectItemIndicator, SelectItemText } from 'reka-ui'
import { Check } from 'lucide-vue-next'
import { cn } from '@/components/shadcn_ui/lib/utils'
const props = defineProps({
    value: { type: [String, Number, Object], required: true },
    disabled: Boolean,
    class: { type: [String, Array, Object], default: '' },
})
</script>

<template>
    <SelectItem
        :value="props.value"
        :disabled="props.disabled"
        :class="cn('relative flex w-full cursor-default select-none items-center rounded-sm py-1.5 pl-7 pr-2 text-xs outline-none transition-colors focus:bg-accent focus:text-accent-foreground data-[disabled]:pointer-events-none data-[disabled]:opacity-50', props.class)"
    >
        <span class="absolute left-1.5 flex size-4 items-center justify-center">
            <SelectItemIndicator>
                <Check class="size-3.5" />
            </SelectItemIndicator>
        </span>
        <SelectItemText>
            <slot />
        </SelectItemText>
    </SelectItem>
</template>
