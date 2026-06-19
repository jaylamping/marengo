import type { ReactNode } from 'react';

import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  type cardVariants,
} from '@/components/ui/card';
import type { VariantProps } from 'class-variance-authority';
import { cn } from '@/lib/utils';

type DashboardCardShellProps = {
  description?: string;
  title: ReactNode;
  titleClassName?: string;
  action?: ReactNode;
  content?: ReactNode;
  footerPrimary: ReactNode;
  footerSecondary?: ReactNode;
  className?: string;
  variant?: VariantProps<typeof cardVariants>['variant'];
};

export function DashboardCardShell({
  description,
  title,
  titleClassName,
  action,
  content,
  footerPrimary,
  footerSecondary,
  className,
  variant = 'glass',
}: DashboardCardShellProps) {
  return (
    <Card variant={variant} className={cn('@container/card', className)}>
      <CardHeader>
        {description ? <CardDescription>{description}</CardDescription> : null}
        <CardTitle className={cn('text-lg font-semibold', titleClassName)}>
          {title}
        </CardTitle>
        {action ? <CardAction>{action}</CardAction> : null}
      </CardHeader>
      {content ? <CardContent className="px-6 pb-0">{content}</CardContent> : null}
      <CardFooter className="flex-col items-start gap-1.5 text-sm">
        <div className="line-clamp-1 font-medium">{footerPrimary}</div>
        {footerSecondary ? (
          <div className="text-muted-foreground">{footerSecondary}</div>
        ) : null}
      </CardFooter>
    </Card>
  );
}
