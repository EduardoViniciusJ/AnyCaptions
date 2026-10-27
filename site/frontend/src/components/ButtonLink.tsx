import type { ComponentProps } from 'react';
import type { VariantProps } from 'class-variance-authority';
import { Button, buttonVariants } from '@/components/ui/button';

type Props = ComponentProps<'a'> & VariantProps<typeof buttonVariants>;

export function ButtonLink({ variant, size, className, children, ...props }: Props) {
  return (
    <Button asChild variant={variant} size={size} className={className}>
      <a {...props}>{children}</a>
    </Button>
  );
}
